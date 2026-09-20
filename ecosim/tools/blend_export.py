"""Export a tagged Blender scene to an ecosim world bundle (version 2).

    blender -b <scene.blend> --python ecosim/tools/blend_export.py -- <out_dir> [--audit N]

`../docs/SCENE-CONTRACT.md` is authoritative for both the tags this reads and the bundle it
writes; this file implements its "Bundle v2" section and nothing more.

Everything down to the `bpy` guard is pure Python. `python -m unittest discover -s tools -t tools`
runs those parts without Blender, which is how CI checks the rasteriser: CI has no Blender and
never needs one, because the exported bundle is committed under `ecosim/worlds/`.

The export is deterministic: the same `.blend` must always give a byte-identical bundle. That
means no `set`/`dict` iteration over scene data (objects are taken in name order), a rasteriser
whose result does not depend on triangle order (a maximum), positions rounded to millimetres, and
JSON written with a fixed key order and LF newlines.
"""

import array
import json
import math
import os
import sys
from collections import Counter, namedtuple

# --- the contract, repeated here because this file is the only writer of it ----------------------

BUNDLE_FORMAT = "ecosim-world-bundle"
BUNDLE_VERSION = 2

#: Every medium, in the contract's order; the index is the code written to `medium.u8`.
MEDIA = ("soil", "lawn", "bed", "mulch", "gravel", "concrete", "asphalt", "roof", "water")

#: Barycentric slack, so a cell centre exactly on a triangle's edge or vertex counts as covered.
EPS = 1e-9

#: The ground grid a bundle is written on: `width` x `depth` cells of `cell_m`, cell (0, 0) at the
#: south-west corner (`ox`, `oy`) in scene coordinates. Cell (i, j)'s centre is at
#: (ox + (i + 0.5)·cell_m, oy + (j + 0.5)·cell_m), and the flat index is `i + width·j`.
Grid = namedtuple("Grid", "width depth cell_m ox oy")


def grid_of(size_m, cell_m, origin):
    """The ground grid of a square `size_m` crop at `cell_m` cells, with `origin` its SW corner."""
    if not (cell_m > 0.0 and size_m > 0.0):
        raise ValueError("eco_size_m and eco_ground_cell_m must be positive, got %r and %r" % (size_m, cell_m))
    n = int(round(size_m / cell_m))
    if abs(n * cell_m - size_m) > 1e-9:
        raise ValueError("eco_ground_cell_m = %r does not divide eco_size_m = %r" % (cell_m, size_m))
    return Grid(n, n, float(cell_m), float(origin[0]), float(origin[1]))


def mm(v):
    """`v` in millimetres of precision, with no negative zero (it would differ byte for byte)."""
    r = round(float(v), 3)
    return r if r else 0.0


# --- rasterising ---------------------------------------------------------------------------------


def rasterise_tops(tris, grid):
    """Top z at each ground-cell centre over a triangle soup, or None where nothing covers it.

    This is the downward ray cast the contract asks for, done the other way round: instead of one
    ray per cell, each triangle fills the cells its XY projection covers and the highest z at a
    cell wins. A maximum does not depend on the order triangles arrive in, and a cell centre that
    lands exactly on a shared vertex — which at the Capitol is *every* ground cell, since the
    ground mesh's vertices are the cell centres — is covered by all the triangles meeting there
    rather than at the mercy of a ray-triangle tie. Triangles with no XY area (the vertical walls
    of a building) have no footprint and are skipped.
    """
    w, d, cell, ox, oy = grid.width, grid.depth, grid.cell_m, grid.ox, grid.oy
    tops = [None] * (w * d)
    for (ax, ay, az), (bx, by, bz), (cx, cy, cz) in tris:
        det = (by - cy) * (ax - cx) + (cx - bx) * (ay - cy)
        if det == 0.0:
            continue
        i0 = max(0, int(math.ceil((min(ax, bx, cx) - EPS - ox) / cell - 0.5)))
        i1 = min(w - 1, int(math.floor((max(ax, bx, cx) + EPS - ox) / cell - 0.5)))
        j0 = max(0, int(math.ceil((min(ay, by, cy) - EPS - oy) / cell - 0.5)))
        j1 = min(d - 1, int(math.floor((max(ay, by, cy) + EPS - oy) / cell - 0.5)))
        for j in range(j0, j1 + 1):
            py = oy + (j + 0.5) * cell
            row = j * w
            for i in range(i0, i1 + 1):
                px = ox + (i + 0.5) * cell
                u = ((by - cy) * (px - cx) + (cx - bx) * (py - cy)) / det
                if u < -EPS:
                    continue
                v = ((cy - ay) * (px - cx) + (ax - cx) * (py - cy)) / det
                if v < -EPS or u + v > 1.0 + EPS:
                    continue
                z = u * az + v * bz + (1.0 - u - v) * cz
                k = row + i
                if tops[k] is None or z > tops[k]:
                    tops[k] = z
    return tops


def merge_surfaces(grid, layers, default_medium, ground_h):
    """Medium code and building height per ground cell.

    `layers` is [(medium name, tops)] in the order the objects were read, one entry per tagged
    surface. The surface with the higher top at a cell wins; an exact tie keeps the earlier layer,
    so the caller's order (object name) settles it. Cells no surface covers get `default_medium`.
    A cell whose winner is a roof also records its building height above the ground there, never
    below zero — a wall dipping under the terrain is a modelling artefact, not a negative building.
    """
    n = grid.width * grid.depth
    if len(ground_h) != n:
        raise ValueError("ground height grid is %d cells, expected %d" % (len(ground_h), n))
    medium = bytearray([MEDIA.index(default_medium)]) * n
    building_h = [0.0] * n
    best = [None] * n
    roof = MEDIA.index("roof")
    for name, tops in layers:
        code = MEDIA.index(name)
        if len(tops) != n:
            raise ValueError("surface %r covers %d cells, expected %d" % (name, len(tops), n))
        for k, z in enumerate(tops):
            if z is None or (best[k] is not None and z <= best[k]):
                continue
            best[k] = z
            medium[k] = code
            building_h[k] = max(0.0, z - ground_h[k]) if code == roof else 0.0
    return medium, building_h


# --- entities ------------------------------------------------------------------------------------


def tree_row(x, y, height, crown_radius, crown_base):
    """One `trees.json` row, keys in the contract's order and lengths rounded to millimetres."""
    return {
        "x": mm(x),
        "y": mm(y),
        "height": mm(height),
        "crown_radius": mm(crown_radius),
        "crown_base": mm(min(crown_base, height)),
    }


def shrub_row(x, y, height, rx, ry, angle):
    """One `shrubs.json` row. The angle is radians, kept to a microradian."""
    a = round(float(angle), 6)
    return {"x": mm(x), "y": mm(y), "height": mm(height), "rx": mm(rx), "ry": mm(ry), "angle": a if a else 0.0}


def pipe_row(pipe_id, inlet, outlet, capacity_m3h, illustrative):
    """One `pipes.json` row."""
    return {
        "id": pipe_id,
        "inlet": [mm(inlet[0]), mm(inlet[1])],
        "outlet": [mm(outlet[0]), mm(outlet[1])],
        "capacity_m3h": mm(capacity_m3h),
        "illustrative": bool(illustrative),
    }


def sort_entities(named_rows, key=lambda r: (r["y"], r["x"])):
    """Rows of `(object name, row)` south to north, then west to east, then by name.

    The name is only the tie-break, so two plants at the same rounded position keep a stable order
    without the scene's object order ever reaching the bundle.
    """
    return [row for _, row in sorted(named_rows, key=lambda nr: (key(nr[1]), nr[0]))]


def pipe_key(row):
    """Pipes are ordered by their inlet, which is the end that sits in the world."""
    return (row["inlet"][1], row["inlet"][0])


# --- writing -------------------------------------------------------------------------------------


def f32le(values):
    """`values` as little-endian f32, whatever the host's byte order is."""
    a = array.array("f", values)
    if sys.byteorder != "little":
        a.byteswap()
    return a.tobytes()


def json_rows(rows):
    """A JSON array with one object per line: a diff then shows one entity per line."""
    if not rows:
        return "[]\n"
    body = ",\n".join("  " + json.dumps(r, separators=(", ", ": ")) for r in rows)
    return "[\n" + body + "\n]\n"


def bundle_json(name, size_m, grid, source, counts):
    """`bundle.json`'s object, keys in the contract's order. Every medium is always listed, so a
    medium code means the same thing in every bundle whether or not the scene used it."""
    return {
        "format": BUNDLE_FORMAT,
        "version": BUNDLE_VERSION,
        "name": name,
        "size_m": mm(size_m),
        "ground_cell_m": mm(grid.cell_m),
        "ground_width": grid.width,
        "ground_depth": grid.depth,
        "media": list(MEDIA),
        "source": source,
        "counts": counts,
    }


def write_text(path, text):
    """Write `text` with LF newlines, so a bundle written on Windows matches one written on Linux."""
    with open(path, "w", encoding="utf-8", newline="\n") as f:
        f.write(text)


def write_bundle(out_dir, meta, ground_h, medium, building_h, trees, shrubs, pipes):
    """Write the seven files of a bundle directory."""
    os.makedirs(out_dir, exist_ok=True)
    write_text(os.path.join(out_dir, "bundle.json"), json.dumps(meta, indent=2) + "\n")
    for name, blob in (
        ("ground_h.f32", f32le(ground_h)),
        ("medium.u8", bytes(medium)),
        ("building_h.f32", f32le(building_h)),
    ):
        with open(os.path.join(out_dir, name), "wb") as f:
            f.write(blob)
    write_text(os.path.join(out_dir, "trees.json"), json_rows(trees))
    write_text(os.path.join(out_dir, "shrubs.json"), json_rows(shrubs))
    write_text(os.path.join(out_dir, "pipes.json"), json_rows(pipes))


# --- the Blender half ----------------------------------------------------------------------------

try:
    import bpy
    from mathutils import Vector
    from mathutils.bvhtree import BVHTree
except ImportError:  # imported by the unit tests, which only use the pure parts above
    bpy = None


def scene_prop(scene, key, cast=None):
    """A required scene custom property, with the contract's name in the error."""
    v = scene.get(key)
    if v is None:
        raise SystemExit("%s: the scene has no %s (see docs/SCENE-CONTRACT.md)" % (scene.name, key))
    return cast(v) if cast else v


def tagged(kind):
    """Every object with `eco_kind == kind`, in name order so the export cannot depend on the
    scene's internal ordering."""
    return [ob for ob in sorted(bpy.data.objects, key=lambda o: o.name) if ob.get("eco_kind") == kind]


def world_tris(ob, depsgraph):
    """The object's evaluated mesh as world-space triangles."""
    evaluated = ob.evaluated_get(depsgraph)
    me = evaluated.to_mesh()
    try:
        m = evaluated.matrix_world
        vs = [(m @ v.co)[:] for v in me.vertices]
        try:
            me.calc_loop_triangles()
        except AttributeError:
            pass
        loops = getattr(me, "loop_triangles", ())
        if len(loops):
            return [(vs[t.vertices[0]], vs[t.vertices[1]], vs[t.vertices[2]]) for t in loops]
        out = []  # a mesh Blender would not triangulate for us: fan the polygons
        for p in me.polygons:
            idx = list(p.vertices)
            out.extend((vs[idx[0]], vs[idx[k]], vs[idx[k + 1]]) for k in range(1, len(idx) - 1))
        return out
    finally:
        evaluated.to_mesh_clear()


def box(ob):
    """The object's world-space bounding box as (min, max) triples."""
    pts = [(ob.matrix_world @ Vector(c))[:] for c in ob.bound_box]
    return tuple(min(p[i] for p in pts) for i in range(3)), tuple(max(p[i] for p in pts) for i in range(3))


def audit_rays(ob, depsgraph, grid, tops, samples):
    """Cross-check `rasterise_tops` against Blender's own ray caster on `samples` ground cells.

    The rasteriser is the export's source of truth (it is deterministic and CI can test it without
    Blender), so this is a check and not a second implementation: it casts a real downward BVHTree
    ray at evenly spaced cell centres and reports how far the two disagree.
    """
    evaluated = ob.evaluated_get(depsgraph)
    me = evaluated.to_mesh()
    try:
        m = evaluated.matrix_world
        verts = [(m @ v.co)[:] for v in me.vertices]
        polys = [tuple(p.vertices) for p in me.polygons]
        bvh = BVHTree.FromPolygons(verts, polys)
    finally:
        evaluated.to_mesh_clear()
    top_z = max(v[2] for v in verts) + 10.0
    down = Vector((0.0, 0.0, -1.0))
    n = grid.width * grid.depth
    step = max(1, n // max(1, samples))
    hits, misses, worst = 0, 0, 0.0
    for k in range(0, n, step):
        i, j = k % grid.width, k // grid.width
        p = Vector((grid.ox + (i + 0.5) * grid.cell_m, grid.oy + (j + 0.5) * grid.cell_m, top_z))
        loc = bvh.ray_cast(p, down)[0]
        if loc is None or tops[k] is None:
            misses += 1
            continue
        hits += 1
        worst = max(worst, abs(loc.z - tops[k]))
    return hits, misses, worst


def export(scene, out_dir, samples):
    """Read the tagged scene and write the bundle. Returns the summary printed by `main`."""
    depsgraph = bpy.context.evaluated_depsgraph_get()
    name = scene_prop(scene, "eco_name", str)
    origin = [float(v) for v in scene_prop(scene, "eco_origin")]
    grid = grid_of(float(scene_prop(scene, "eco_size_m")), float(scene_prop(scene, "eco_ground_cell_m")), origin)
    default_medium = scene_prop(scene, "eco_default_medium", str)
    if default_medium not in MEDIA:
        raise SystemExit("eco_default_medium = %r is not a medium" % default_medium)
    source = scene_prop(scene, "eco_source", str)

    grounds = tagged("ground")
    if len(grounds) != 1:
        raise SystemExit("the scene must have exactly one object tagged ground, found %d" % len(grounds))
    raw = rasterise_tops(world_tris(grounds[0], depsgraph), grid)
    holes = [k for k, z in enumerate(raw) if z is None]
    if holes:
        k = holes[0]
        raise SystemExit(
            "%s does not cover %d of the %d ground cells, the first at (%d, %d)"
            % (grounds[0].name, len(holes), len(raw), k % grid.width, k // grid.width)
        )
    floor = min(raw)
    ground_h = [mm(z - floor) for z in raw]

    layers = []
    for ob in tagged("surface"):
        medium = ob.get("eco_medium")
        if medium not in MEDIA:
            raise SystemExit("%s: eco_medium = %r is not a medium" % (ob.name, medium))
        layers.append((medium, rasterise_tops(world_tris(ob, depsgraph), grid)))
    medium, building_h = merge_surfaces(grid, layers, default_medium, ground_h)
    building_h = [mm(h) for h in building_h]

    trees = []
    for ob in tagged("tree"):
        lo, hi = box(ob)
        h = ob.get("height", hi[2] - lo[2])
        radius = ob.get("crown_radius", max(hi[0] - lo[0], hi[1] - lo[1]) / 2.0)
        base = ob.get("crown_base", 0.0)
        p = ob.matrix_world.translation
        trees.append((ob.name, tree_row(p.x - origin[0], p.y - origin[1], h, radius, base)))
    shrubs = []
    for ob in tagged("shrub"):
        lo, hi = box(ob)
        p = ob.matrix_world.translation
        shrubs.append(
            (
                ob.name,
                shrub_row(
                    p.x - origin[0],
                    p.y - origin[1],
                    ob.get("height", hi[2] - lo[2]),
                    ob.get("rx", (hi[0] - lo[0]) / 2.0),
                    ob.get("ry", (hi[1] - lo[1]) / 2.0),
                    ob.get("angle", 0.0),
                ),
            )
        )
    pipes = []
    for ob in tagged("pipe"):
        pts = spline_points(ob)
        if len(pts) < 2:
            raise SystemExit("%s: a pipe needs a curve with at least two points" % ob.name)
        ends = [(p[0] - origin[0], p[1] - origin[1]) for p in (pts[0], pts[-1])]
        pipes.append(
            (ob.name, pipe_row(ob.name, ends[0], ends[1], ob.get("capacity_m3h", 0.0), ob.get("eco_illustrative", False)))
        )

    trees = sort_entities(trees)
    shrubs = sort_entities(shrubs)
    pipes = sort_entities(pipes, key=pipe_key)
    counts = {"trees": len(trees), "shrubs": len(shrubs), "pipes": len(pipes)}
    meta = bundle_json(name, grid.width * grid.cell_m, grid, source, counts)
    write_bundle(out_dir, meta, ground_h, medium, building_h, trees, shrubs, pipes)

    cells = Counter(MEDIA[c] for c in medium)
    lines = [
        "wrote %s: %s, %d m at %g m cells (%d x %d ground)"
        % (out_dir, name, grid.width * grid.cell_m, grid.cell_m, grid.width, grid.depth),
        "ground height %.3f..%.3f m above the crop minimum (scene z %.3f)" % (min(ground_h), max(ground_h), floor),
        "surfaces %d, cells per medium %s" % (len(layers), dict(sorted(cells.items()))),
        "building height 0..%.3f m over %d cells" % (max(building_h), sum(1 for h in building_h if h > 0.0)),
        "entities %s" % counts,
    ]
    if samples:
        hits, misses, worst = audit_rays(grounds[0], depsgraph, grid, raw, samples)
        lines.append("ray-cast audit: %d cells hit, %d missed, worst disagreement %.2e m" % (hits, misses, worst))
        if worst > 1e-3:
            raise SystemExit("ray-cast audit: the rasteriser and Blender's ray caster differ by %.4f m" % worst)
    return "\n".join(lines)


def spline_points(ob):
    """A curve object's first spline as world-space points."""
    if ob.type != "CURVE" or not ob.data.splines:
        raise SystemExit("%s: a pipe must be a curve" % ob.name)
    s = ob.data.splines[0]
    m = ob.matrix_world
    pts = [p.co for p in s.bezier_points] if s.type == "BEZIER" else [p.co.to_3d() for p in s.points]
    return [(m @ p)[:] for p in pts]


def main(argv):
    """`<out_dir> [--audit N]`, taken from Blender's arguments after `--`."""
    if not argv or argv[0].startswith("-"):
        raise SystemExit("usage: blender -b <scene.blend> --python blend_export.py -- <out_dir> [--audit N]")
    samples = 4096
    if "--audit" in argv:
        samples = int(argv[argv.index("--audit") + 1])
    print(export(bpy.context.scene, argv[0], samples))


if __name__ == "__main__" and bpy is not None:
    args = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []
    main(args)

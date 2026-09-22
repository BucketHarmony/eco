"""The Blender exporter's pure half, in plain CPython.

    python -m unittest discover -s tools -t tools

CI runs this (`just pyexport`, step 0): the exporter's rasteriser decides what the committed
Capitol bundle contains, and CI has no Blender to re-run the export with. The tests use no
third-party package, so they run wherever Python does.
"""

import json
import unittest

import blend_export as bx


def square(x0, y0, x1, y1, z):
    """Two triangles covering the rectangle at height `z`."""
    a, b, c, d = (x0, y0, z), (x1, y0, z), (x1, y1, z), (x0, y1, z)
    return [(a, b, c), (a, c, d)]


def covered(tops, grid):
    """The (i, j) of every cell the rasteriser marked, in index order."""
    return [(k % grid.width, k // grid.width) for k, z in enumerate(tops) if z is not None]


class Rasterise(unittest.TestCase):
    """A 4 m square of 0.5 m cells, its south-west corner at the origin."""

    grid = bx.grid_of(4.0, 0.5, (0.0, 0.0))

    def test_a_square_fills_the_cells_whose_centres_it_covers(self):
        tops = bx.rasterise_tops(square(1.0, 1.0, 3.0, 3.0, 2.5), self.grid)
        self.assertEqual(covered(tops, self.grid), [(i, j) for j in range(2, 6) for i in range(2, 6)])
        self.assertEqual({z for z in tops if z is not None}, {2.5})

    def test_a_square_on_cell_centres_takes_the_cells_on_its_edge(self):
        # Centres lie at 0.25, 0.75, ...; an edge exactly through a centre still covers it, which
        # is what makes a ground mesh whose vertices are the cell centres hole-free.
        tops = bx.rasterise_tops(square(0.75, 0.75, 1.75, 1.75, 1.0), self.grid)
        self.assertEqual(covered(tops, self.grid), [(i, j) for j in range(1, 4) for i in range(1, 4)])

    def test_an_l_shape_leaves_its_notch_empty(self):
        tris = square(0.0, 0.0, 2.0, 1.0, 1.0) + square(0.0, 1.0, 1.0, 2.0, 1.0)
        tops = bx.rasterise_tops(tris, self.grid)
        want = [(i, j) for j in range(0, 2) for i in range(0, 4)] + [(i, j) for j in range(2, 4) for i in range(0, 2)]
        self.assertEqual(covered(tops, self.grid), sorted(want, key=lambda p: (p[1], p[0])))

    def test_a_tilted_triangle_interpolates_its_height(self):
        tris = [((0.0, 0.0, 0.0), (4.0, 0.0, 4.0), (4.0, 4.0, 4.0))]
        tops = bx.rasterise_tops(tris, self.grid)
        # On the diagonal the plane is z = x, so the cell centred at (3.75, 0.25) is 3.75 high.
        self.assertAlmostEqual(tops[7 + self.grid.width * 0], 3.75)

    def test_the_higher_top_wins_an_overlap_whatever_the_order(self):
        low = bx.rasterise_tops(square(0.0, 0.0, 4.0, 4.0, 1.0), self.grid)
        high = bx.rasterise_tops(square(1.0, 1.0, 3.0, 3.0, 9.0), self.grid)
        flat = [0.0] * (self.grid.width * self.grid.depth)
        first, _ = bx.merge_surfaces(self.grid, [("lawn", low), ("roof", high)], "soil", flat)
        second, _ = bx.merge_surfaces(self.grid, [("roof", high), ("lawn", low)], "soil", flat)
        self.assertEqual(bytes(first), bytes(second))
        self.assertEqual(first[0], bx.MEDIA.index("lawn"))
        self.assertEqual(first[3 + self.grid.width * 3], bx.MEDIA.index("roof"))

    def test_an_exact_tie_keeps_the_earlier_layer(self):
        a = bx.rasterise_tops(square(0.0, 0.0, 4.0, 4.0, 1.0), self.grid)
        b = bx.rasterise_tops(square(0.0, 0.0, 4.0, 4.0, 1.0), self.grid)
        medium, _ = bx.merge_surfaces(self.grid, [("asphalt", a), ("concrete", b)], "soil", [0.0] * 64)
        self.assertEqual(set(medium), {bx.MEDIA.index("asphalt")})

    def test_uncovered_cells_take_the_default_medium(self):
        tops = bx.rasterise_tops(square(0.0, 0.0, 1.0, 1.0, 1.0), self.grid)
        medium, _ = bx.merge_surfaces(self.grid, [("concrete", tops)], "lawn", [0.0] * 64)
        self.assertEqual(medium[0], bx.MEDIA.index("concrete"))
        self.assertEqual(medium[self.grid.width * self.grid.depth - 1], bx.MEDIA.index("lawn"))

    def test_building_height_is_the_roof_above_the_ground_and_never_negative(self):
        roof = bx.rasterise_tops(square(0.0, 0.0, 2.0, 4.0, 3.0), self.grid)
        ground = [2.0] * 32 + [5.0] * 32  # the north half stands above this roof
        medium, building = bx.merge_surfaces(self.grid, [("roof", roof)], "lawn", ground)
        self.assertEqual(building[0], 1.0)
        self.assertEqual(building[self.grid.width * 4], 0.0)
        self.assertEqual(building[7], 0.0)  # no roof there, so no building
        self.assertEqual(medium[7], bx.MEDIA.index("lawn"))

    def test_vertical_walls_have_no_footprint(self):
        wall = [((0.0, 0.0, 0.0), (4.0, 0.0, 0.0), (4.0, 0.0, 9.0))]
        self.assertEqual(bx.rasterise_tops(wall, self.grid), [None] * 64)

    def test_a_cell_size_that_does_not_divide_the_crop_is_refused(self):
        with self.assertRaises(ValueError):
            bx.grid_of(4.0, 0.3, (0.0, 0.0))


class Rounding(unittest.TestCase):
    def test_lengths_round_to_a_millimetre(self):
        self.assertEqual(bx.mm(1.23456), 1.235)
        self.assertEqual(bx.mm(-2.0 / 3.0), -0.667)

    def test_negative_zero_becomes_zero(self):
        self.assertEqual(repr(bx.mm(-0.0001)), "0.0")
        self.assertEqual(repr(bx.mm(-0.0)), "0.0")
        self.assertEqual(repr(bx.shrub_row(0, 0, 1, 1, 1, -1e-9)["angle"]), "0.0")

    def test_a_crown_base_cannot_stand_above_its_tree(self):
        self.assertEqual(bx.tree_row(0, 0, 4.0, 1.0, 9.0)["crown_base"], 4.0)


class Ordering(unittest.TestCase):
    def test_entities_run_south_to_north_then_west_to_east(self):
        rows = [
            ("Tree_b", bx.tree_row(9.0, 2.0, 1, 1, 0)),
            ("Tree_c", bx.tree_row(1.0, 5.0, 1, 1, 0)),
            ("Tree_a", bx.tree_row(3.0, 2.0, 1, 1, 0)),
        ]
        self.assertEqual([(r["x"], r["y"]) for r in bx.sort_entities(rows)], [(3.0, 2.0), (9.0, 2.0), (1.0, 5.0)])

    def test_the_object_name_breaks_a_tie(self):
        rows = [("Shrub_9", bx.tree_row(1, 1, 1, 1, 0)), ("Shrub_1", bx.tree_row(1, 1, 1, 1, 0))]
        self.assertEqual(bx.sort_entities(rows), bx.sort_entities(list(reversed(rows))))

    def test_pipes_are_ordered_by_their_inlet(self):
        rows = [
            ("b", bx.pipe_row("b", (5.0, 9.0), (5.0, 16.0), 50, True)),
            ("a", bx.pipe_row("a", (1.0, 2.0), (0.0, 2.0), 50, True)),
        ]
        self.assertEqual([r["id"] for r in bx.sort_entities(rows, key=bx.pipe_key)], ["a", "b"])


class Layout(unittest.TestCase):
    """The bytes on disk, which are the contract with `ecosim::bundle`."""

    def test_an_entity_file_is_one_object_per_line_in_the_contract_s_key_order(self):
        text = bx.json_rows([bx.tree_row(1.5, 2.25, 12.0, 3.5, 4.0), bx.tree_row(0.001, 0.0, 1.0, 0.5, 0.0)])
        self.assertEqual(
            text,
            "[\n"
            '  {"x": 1.5, "y": 2.25, "height": 12.0, "crown_radius": 3.5, "crown_base": 4.0},\n'
            '  {"x": 0.001, "y": 0.0, "height": 1.0, "crown_radius": 0.5, "crown_base": 0.0}\n'
            "]\n",
        )

    def test_an_empty_entity_file_is_an_empty_array(self):
        self.assertEqual(bx.json_rows([]), "[]\n")

    def test_a_pipe_keeps_its_two_element_ends(self):
        text = bx.json_rows([bx.pipe_row("pipe_1", (1.0, 2.0), (0.0, 2.0), 50.0, True)])
        self.assertEqual(
            text,
            '[\n  {"id": "pipe_1", "inlet": [1.0, 2.0], "outlet": [0.0, 2.0], '
            '"capacity_m3h": 50.0, "illustrative": true}\n]\n',
        )

    def test_bundle_json_names_every_medium_with_soil_first(self):
        grid = bx.grid_of(16.0, 0.5, (-8.0, -8.0))
        meta = bx.bundle_json("x", 16.0, grid, "s", 42.73365, {"trees": 0, "shrubs": 0, "pipes": 0})
        self.assertEqual(
            list(meta),
            "format version name size_m ground_cell_m ground_width ground_depth media source latitude_deg counts".split(),
        )
        self.assertEqual(meta["media"][0], "soil")
        self.assertEqual((meta["version"], meta["ground_width"], meta["ground_depth"]), (2, 32, 32))
        self.assertEqual(json.loads(json.dumps(meta))["format"], "ecosim-world-bundle")

    def test_latitude_is_rounded_and_range_checked(self):
        # Rounded to 1e-6 of a degree, and -0.0 written as 0.0 so the bytes cannot differ.
        self.assertEqual(bx.latitude(42.7336512345), 42.733651)
        self.assertEqual(bx.latitude(-0.0000001), 0.0)
        self.assertEqual(repr(bx.latitude(-0.0000001)), "0.0")
        self.assertEqual(bx.latitude(-33.8688), -33.8688)
        for bad in (90.5, -91.0, 1000.0):
            with self.assertRaises(ValueError):
                bx.latitude(bad)

    def test_f32_is_little_endian(self):
        self.assertEqual(bx.f32le([1.0, -2.5]).hex(), "0000803f000020c0")

    def test_the_module_imports_without_blender(self):
        self.assertIsNone(bx.bpy, "the pure half must import outside Blender")


if __name__ == "__main__":
    unittest.main()

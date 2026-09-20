## Measured tables

### Runoff fraction by surface, flat single-medium worlds, 20000 ticks

| surface | storm mean (mm) | runoff fraction | of storms above the mean | soil water at the end (mm) |
|---|---|---|---|---|
| lawn | 5 | 0.051 | 0.063 | 148.0 |
| lawn | 10 | 0.207 | 0.272 | 143.9 |
| lawn | 20 | 0.485 | 0.639 | 147.9 |
| lawn | 40 | 0.710 | 0.935 | 124.6 |
| street | 5 | 1.000 | 1.000 | 0.0 |
| street | 10 | 1.000 | 1.000 | 0.0 |
| street | 20 | 1.000 | 1.000 | 0.0 |
| street | 40 | 1.000 | 1.000 | 0.0 |
| roof | 5 | 1.000 | 1.000 | 0.0 |
| roof | 10 | 1.000 | 1.000 | 0.0 |
| roof | 20 | 1.000 | 1.000 | 0.0 |
| roof | 40 | 1.000 | 1.000 | 0.0 |

### The Capitol, 20000 ticks, seed 42

| storm mean (mm) | storms | rain (mm) | runoff | outflow | drainage (mm) | largest storm |
|---|---|---|---|---|---|---|
| 5 | 3994 | 19657 | 40.3% | 28.7% | 10668 | 39.5 mm @ 19186 |
| 10 | 2014 | 20110 | 48.3% | 36.1% | 9493 | 68.4 mm @ 2117 |
| 20 | 1018 | 21353 | 61.5% | 47.6% | 7912 | 131.5 mm @ 19362 |
| 40 | 486 | 20775 | 76.0% | 61.8% | 4857 | 221.6 mm @ 9465 |

### Ponding after the largest storm (221.6 mm at tick 9465, `rain.storm_mean_mm=40`)

Ponded volume over the whole 256 m site: 1323.8 m3 in 47384 of 262144 ground cells.

| rank | volume (m3) | cells | deepest (mm) | at (m) | medium |
|---|---|---|---|---|---|
| 1 | 977.24 | 5989 | 5324 | (116.5, 51.5) | lawn |
| 2 | 90.59 | 1201 | 1137 | (91.5, 186.0) | lawn |
| 3 | 31.47 | 1959 | 189 | (129.5, 216.0) | lawn |
| 4 | 17.33 | 476 | 479 | (108.0, 94.5) | lawn |
| 5 | 12.07 | 1121 | 206 | (156.5, 94.5) | lawn |

### Lawn cover by soil water, the default storm size, tick 20000 (683 lawn patches)

| quarter | patches | soil water (mm) | grass | shrub | trees |
|---|---|---|---|---|---|
| driest quarter | 170 | 140.0 | 0.545 | 0.489 | 1137 |
| wettest quarter | 170 | 146.4 | 0.671 | 0.017 | 0 |

### Cost of the storm pass at 512 x 512

2000 ticks of the Capitol with a storm every tick: 6670 ms; with no storms: 962 ms. One storm pass over 262144 ground cells costs **2.85 ms**.

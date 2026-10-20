| stage | λ128 1K | λ128 4K | λ128 32K | λ100 1K | λ100 4K | λ100 32K |
|---|---|---|---|---|---|---|
| keyed ring | 79 | 76 | 79 | 73 | 73 | 73 |
| norm sumcheck | 270 | 249 | 267 | 119 | 116 | 131 |
| binder sumcheck | 258 | 275 | 293 | 133 | 135 | 153 |
| bridge (product forest) | 1,325 | 1,485 | 1,725 | 529 | 593 | 689 |
| joint sumcheck | 218 | 236 | 263 | 74 | 80 | 89 |
| opening: ring switch + L0 folds | 104 | 104 | 104 | 53 | 53 | 53 |
| opening: level 0 | 3,243 | 3,784 | 4,777 | 2,631 | 3,073 | 3,796 |
| opening: recursive levels | 2,373 | 3,892 | 6,274 | 1,879 | 3,067 | 4,908 |
| opening: final level | 1,158 | 2,160 | 4,204 | 1,112 | 2,121 | 4,168 |
| **total** | **9,028** | **12,261** | **17,986** | **6,603** | **9,311** | **14,060** |
| of which transcript | 2,485 (28 %) | 2,714 (22 %) | 3,069 (17 %) | 1,376 (21 %) | 1,503 (16 %) | 1,710 (12 %) |
| of which proof of work | 386 (4 %) | 434 (4 %) | 504 (3 %) | 0 (0 %) | 0 (0 %) | 0 (0 %) |
| of which Merkle | 6,157 (68 %) | 9,113 (74 %) | 14,413 (80 %) | 5,227 (79 %) | 7,808 (84 %) | 12,350 (88 %) |

| stage | kind | λ128 1K | λ128 4K | λ128 32K | λ100 1K | λ100 4K | λ100 32K |
|---|---|---|---|---|---|---|---|
| keyed ring | transcript | 78 | 75 | 78 | 73 | 73 | 73 |
| keyed ring | proof of work | 1 | 1 | 1 | 0 | 0 | 0 |
| norm | transcript | 249 | 226 | 241 | 119 | 116 | 131 |
| norm | proof of work | 21 | 23 | 26 | 0 | 0 | 0 |
| binder | transcript | 236 | 251 | 266 | 133 | 135 | 153 |
| binder | proof of work | 22 | 24 | 27 | 0 | 0 | 0 |
| bridge | transcript | 1060 | 1188 | 1380 | 529 | 593 | 689 |
| bridge | proof of work | 265 | 297 | 345 | 0 | 0 | 0 |
| joint | transcript | 194 | 210 | 234 | 74 | 80 | 89 |
| joint | proof of work | 24 | 26 | 29 | 0 | 0 | 0 |
| opening: ring switch | transcript | 43 | 43 | 43 | 35 | 35 | 35 |
| opening: ring switch | proof of work | 2 | 2 | 2 | 0 | 0 | 0 |
| opening: L0 folds | transcript | 47 | 47 | 47 | 18 | 18 | 18 |
| opening: L0 folds | proof of work | 12 | 12 | 12 | 0 | 0 | 0 |
| opening: L0 opening | transcript | 166 | 166 | 166 | 127 | 126 | 126 |
| opening: L0 opening | proof of work | 3 | 3 | 3 | 0 | 0 | 0 |
| opening: L0 opening | Merkle | 3074 | 3615 | 4608 | 2504 | 2947 | 3670 |
| opening: level 1 | transcript | 151 | 148 | 147 | 97 | 96 | 96 |
| opening: level 1 | proof of work | 14 | 14 | 14 | 0 | 0 | 0 |
| opening: level 1 | Merkle | 1277 | 1639 | 2198 | 1046 | 1334 | 1743 |
| opening: level 2 | transcript | 135 | 128 | 126 | 82 | 80 | 79 |
| opening: level 2 | proof of work | 13 | 13 | 13 | 0 | 0 | 0 |
| opening: level 2 | Merkle | 783 | 1071 | 1558 | 654 | 869 | 1237 |
| opening: final level | transcript | 126 | 107 | 103 | 89 | 74 | 73 |
| opening: final level | proof of work | 9 | 6 | 6 | 0 | 0 | 0 |
| opening: final level | Merkle | 1023 | 2047 | 4095 | 1023 | 2047 | 4095 |
| opening: level 3 | transcript | 0 | 125 | 120 | 0 | 77 | 75 |
| opening: level 3 | proof of work | 0 | 13 | 13 | 0 | 0 | 0 |
| opening: level 3 | Merkle | 0 | 741 | 1140 | 0 | 611 | 904 |
| opening: level 4 | transcript | 0 | 0 | 118 | 0 | 0 | 73 |
| opening: level 4 | proof of work | 0 | 0 | 13 | 0 | 0 | 0 |
| opening: level 4 | Merkle | 0 | 0 | 814 | 0 | 0 | 701 |
| **total** | | **9028** | **12261** | **17986** | **6603** | **9311** | **14060** |

Merkle per level: opened rows x row elements, leaf + node compressions

| level | λ128 1K | λ128 4K | λ128 32K | λ100 1K | λ100 4K | λ100 32K |
|---|---|---|---|---|---|---|
| L0 | 309x16: 1236 + 1838 | 309x16: 1236 + 2379 | 309x16: 1236 + 3372 | 242x16: 968 + 1536 | 242x16: 968 + 1979 | 241x16: 964 + 2706 |
| level 1 | 190x8: 380 + 897 | 189x8: 378 + 1261 | 189x8: 378 + 1820 | 148x8: 296 + 750 | 148x8: 296 + 1038 | 148x8: 296 + 1447 |
| level 2 | 156x8: 312 + 471 | 155x8: 310 + 761 | 155x8: 310 + 1248 | 122x8: 244 + 410 | 121x8: 242 + 627 | 121x8: 242 + 995 |
| level 3 | – | 142x8: 284 + 457 | 141x8: 282 + 858 | – | 111x8: 222 + 389 | 110x8: 220 + 684 |
| level 4 | – | – | 135x8: 270 + 544 | – | – | 106x8: 212 + 489 |
| final level | 512x4: 512 + 511 | 1024x2: 1024 + 1023 | 2048x2: 2048 + 2047 | 512x4: 512 + 511 | 1024x2: 1024 + 1023 | 2048x2: 2048 + 2047 |
| **leaves + nodes** | **2440 + 3717** | **3232 + 5881** | **4524 + 9889** | **2020 + 3207** | **2752 + 5056** | **3982 + 8368** |

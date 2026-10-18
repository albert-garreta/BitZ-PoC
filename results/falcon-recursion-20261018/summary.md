| λ | batch | thr | base prove (Blake3) | base* prove (SHA) | rec step | base verify | base* verify | final verify | base proof | base* proof | π | public | rec total | GHz rec/base | RSS rec |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| 128 | 1024 | 10 | 118 ms | 81 ms | 1.18 s | 5.5 ms | 4.8 ms | 0.82 s | 226.8 KB | 223.8 KB | 309.3 KB | 20.9 KB | 330.2 KB | 4.08/3.90 | 9487MiB |
| 128 | 4096 | 10 | 162 ms | 148 ms | 1.29 s | 7.4 ms | 6.2 ms | 0.91 s | 313.0 KB | 313.6 KB | 307.3 KB | 35.5 KB | 342.8 KB | 4.10/3.90 | 9967MiB |
| 128 | 32768 | 10 | 800 ms | 799 ms | 2.94 s | 20.0 ms | 20.9 ms | 3.48 s | 593.2 KB | 593.1 KB | 349.4 KB | 153.7 KB | 503.1 KB | 4.46/3.89 | 12464MiB |
| 100 | 1024 | 10 | 39 ms | 39 ms | 0.93 s | 4.9 ms | 4.3 ms | 0.62 s | 186.0 KB | 183.5 KB | 91.1 KB | 11.5 KB | 102.6 KB | 3.99/4.33 | 6425MiB |
| 100 | 4096 | 10 | 101 ms | 105 ms | 1.26 s | 6.8 ms | 5.8 ms | 0.81 s | 259.4 KB | 258.1 KB | 98.8 KB | 24.6 KB | 123.4 KB | 3.95/4.04 | 10895MiB |
| 100 | 32768 | 10 | 712 ms | 748 ms | 1.90 s | 19.3 ms | 19.6 ms | 1.42 s | 518.7 KB | 517.3 KB | 108.5 KB | 140.9 KB | 249.4 KB | 4.05/3.91 | 12402MiB |
| 128 | 1024 | 1 | 519 ms | 382 ms | 2.99 s | 6.1 ms | 5.6 ms | 1.24 s | 226.8 KB | 223.8 KB | 309.3 KB | 20.9 KB | 330.2 KB | 4.74/4.32 | 9287MiB |
| 128 | 4096 | 1 | 740 ms | 711 ms | 3.32 s | 13.1 ms | 12.3 ms | 1.38 s | 313.0 KB | 313.6 KB | 307.3 KB | 35.5 KB | 342.8 KB | 4.68/4.34 | 8996MiB |
| 128 | 32768 | 1 | 3567 ms | 3490 ms | 5.88 s | 77.7 ms | 77.7 ms | 3.50 s | 593.2 KB | 593.1 KB | 349.4 KB | 153.7 KB | 503.1 KB | 4.56/4.35 | 12296MiB |
| 100 | 1024 | 1 | 123 ms | 127 ms | 2.30 s | 5.6 ms | 5.3 ms | 0.92 s | 186.0 KB | 183.5 KB | 91.1 KB | 11.5 KB | 102.6 KB | 4.65/4.32 | 6138MiB |
| 100 | 4096 | 1 | 407 ms | 414 ms | 3.70 s | 12.3 ms | 12.2 ms | 1.28 s | 259.4 KB | 258.1 KB | 98.8 KB | 24.6 KB | 123.4 KB | 4.68/4.34 | 9954MiB |
| 100 | 32768 | 1 | 3165 ms | 3172 ms | 5.16 s | 76.7 ms | 75.2 ms | 1.91 s | 518.7 KB | 517.3 KB | 108.5 KB | 140.9 KB | 249.4 KB | 4.43/4.35 | 11960MiB |

Recursion step breakdown (s): witness | record (overlaps) | early commit | passes wall | relation | outer witness | FlockRec prove
  λ128 b1024 t10: 0.05 | 0.41 | 0.04 | 0.41 | 0.11 | 0.00 | 0.65   circuit: 8952 SHA blocks, 59.2 M committed bits, 184 M cells, outer f/h bits 27/28; public top ['bridge:796:12736', 'bridge folds:256:4096', 'opening:905:2734', 'binder:52:464']
  λ128 b4096 t10: 0.07 | 0.48 | 0.04 | 0.48 | 0.13 | 0.00 | 0.68   circuit: 12273 SHA blocks, 80.9 M committed bits, 252 M cells, outer f/h bits 27/28; public top ['bridge folds:1024:16384', 'bridge:894:14304', 'opening:1030:3152', 'binder:68:576']
  λ128 b32768 t10: 0.12 | 0.82 | 0.09 | 0.82 | 0.30 | 0.01 | 1.83   circuit: 17897 SHA blocks, 117.9 M committed bits, 367 M cells, outer f/h bits 28/29; public top ['bridge folds:8192:131072', 'bridge:1038:16608', 'opening:1170:4224', 'binder:76:640']
  λ100 b1024 t10: 0.05 | 0.42 | 0.09 | 0.42 | 0.10 | 0.00 | 0.41   circuit: 6488 SHA blocks, 43.1 M committed bits, 133 M cells, outer f/h bits 26/27; public top ['bridge:266:4256', 'bridge folds:256:4096', 'opening:685:1748', 'binder:64:528']
  λ100 b4096 t10: 0.06 | 0.49 | 0.21 | 0.49 | 0.12 | 0.00 | 0.64   circuit: 9266 SHA blocks, 61.3 M committed bits, 190 M cells, outer f/h bits 27/28; public top ['bridge folds:1024:16384', 'bridge:298:4768', 'opening:776:2014', 'binder:66:528']
  λ100 b32768 t10: 0.10 | 0.78 | 0.24 | 0.78 | 0.29 | 0.01 | 0.82   circuit: 14059 SHA blocks, 92.8 M committed bits, 289 M cells, outer f/h bits 27/29; public top ['bridge folds:8192:131072', 'bridge:348:5568', 'opening:878:2691', 'binder:70:560']
  λ128 b1024 t1: 0.05 | 0.39 | 0.13 | 0.57 | 0.19 | 0.01 | 2.22   circuit: 8952 SHA blocks, 59.2 M committed bits, 184 M cells, outer f/h bits 27/28; public top ['bridge:796:12736', 'bridge folds:256:4096', 'opening:905:2734', 'binder:52:464']
  λ128 b4096 t1: 0.07 | 0.47 | 0.13 | 0.66 | 0.23 | 0.01 | 2.43   circuit: 12273 SHA blocks, 80.9 M committed bits, 252 M cells, outer f/h bits 27/28; public top ['bridge folds:1024:16384', 'bridge:894:14304', 'opening:1030:3152', 'binder:68:576']
  λ128 b32768 t1: 0.15 | 0.78 | 0.27 | 1.20 | 0.42 | 0.01 | 4.25   circuit: 17897 SHA blocks, 117.9 M committed bits, 367 M cells, outer f/h bits 28/29; public top ['bridge folds:8192:131072', 'bridge:1038:16608', 'opening:1170:4224', 'binder:76:640']
  λ100 b1024 t1: 0.04 | 0.35 | 0.48 | 0.87 | 0.16 | 0.00 | 1.27   circuit: 6488 SHA blocks, 43.1 M committed bits, 133 M cells, outer f/h bits 26/27; public top ['bridge:266:4256', 'bridge folds:256:4096', 'opening:685:1748', 'binder:64:528']
  λ100 b4096 t1: 0.06 | 0.41 | 0.99 | 1.46 | 0.20 | 0.01 | 2.04   circuit: 9266 SHA blocks, 61.3 M committed bits, 190 M cells, outer f/h bits 27/28; public top ['bridge folds:1024:16384', 'bridge:298:4768', 'opening:776:2014', 'binder:66:528']
  λ100 b32768 t1: 0.14 | 0.73 | 1.01 | 1.88 | 0.41 | 0.01 | 2.86   circuit: 14059 SHA blocks, 92.8 M committed bits, 289 M cells, outer f/h bits 27/29; public top ['bridge folds:8192:131072', 'bridge:348:5568', 'opening:878:2691', 'binder:70:560']

Base proof digests (Blake3, proof_debug_digest):
  λ128 b1024: 964daab7b51f0ead280917be6a71cbb7db01d763141ddc8516899487005f4758 (226848 B, exception counts 2048 B)
  λ128 b4096: 24582a6ab3b6d38a44fe6c8cbf560897634a31108e7c059fcd7476aa7b194009 (312976 B, exception counts 8192 B)
  λ128 b32768: dcb4c8cf0173c2f0686199ae98a56b407d8690b4fe14e1d84e3229239dc01e84 (593232 B, exception counts 65536 B)
  λ100 b1024: 0a82fe730523269a9ad8c412252ec7008ffd9b14574ab5285b9b2c5b625973e2 (185992 B, exception counts 2048 B)
  λ100 b4096: e978371d59274da25d3c0ff362aa43d3753156f11f061d8841349ec8c10bc2cf (259440 B, exception counts 8192 B)
  λ100 b32768: 30e1d5f95e2f25f025bd61b82ecd9be22a352bb6ee94eb620a63795aae160ca1 (518728 B, exception counts 65536 B)

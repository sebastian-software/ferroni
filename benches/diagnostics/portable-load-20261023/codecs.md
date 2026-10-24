# Separate codec diagnostics

These are standalone, in-memory decoding diagnostics completed before the main matrix: CPython gzip/zlib and Node Brotli, nine timed decodes after a round-trip/warmup. Rust load is the median of three Criterion process means. The different runtimes and allocation paths do not establish a direct codec ranking or an application/CDN startup time. No file I/O, network, or host-language bridge is included. Historical Before Brotli timings are omitted.

| Corpus | Variant | Rust load ms | gzip-9 decode ms | Brotli-11 decode ms |
| --- | --- | ---: | ---: | ---: |
| cpp | eager | 92.384 | 1.606 | 1.002 |
| cpp | prepared | 21.890 | 19.908 | 26.347 |
| cpp | portable | 19.835 | 33.315 | 46.240 |
| scss | eager | 49.747 | 0.695 | 0.607 |
| scss | prepared | 11.366 | 10.292 | 16.940 |
| scss | portable | 7.064 | 17.174 | 22.520 |
| java | eager | 5.370 | 0.176 | 0.177 |
| java | prepared | 1.583 | 0.996 | 1.106 |
| java | portable | 1.591 | 1.406 | 1.651 |
| json | eager | 0.172 | 0.022 | 0.025 |
| json | prepared | 0.108 | 0.039 | 0.060 |
| json | portable | 0.117 | 0.037 | 0.065 |
| rust | eager | 2.140 | 0.052 | 0.094 |
| rust | prepared | 0.634 | 0.295 | 0.455 |
| rust | portable | 0.705 | 0.406 | 0.622 |
| html | eager | 43.794 | 3.477 | 2.306 |
| html | prepared | 13.304 | 10.135 | 12.139 |
| html | portable | 11.856 | 14.979 | 19.121 |
| astro | eager | 75.314 | 4.102 | 2.121 |
| astro | prepared | 18.061 | 15.394 | 16.481 |
| astro | portable | 16.446 | 24.330 | 27.921 |
| tsx | eager | 47.590 | 2.367 | 1.003 |
| tsx | prepared | 10.372 | 9.911 | 9.212 |
| tsx | portable | 9.648 | 15.825 | 17.414 |
| c | eager | 14.076 | 0.173 | 0.266 |
| c | prepared | 3.516 | 2.898 | 3.445 |
| c | portable | 3.794 | 5.168 | 4.818 |
| php | eager | 50.533 | 3.364 | 2.766 |
| php | prepared | 15.928 | 10.804 | 14.314 |
| php | portable | 15.131 | 16.179 | 19.864 |

# Divergence results

- **ubuntu-latest**: Linux x86_64, AMD EPYC 7763 64-Core Processor, 2 logical CPUs, SIMD avx2, fma; numpy 2.4.6, pennylane 0.45.1, pennylane-lightning 0.45.0, qiskit 2.6.2, qiskit-aer 0.18.4, vikshep-quantum 0.1.0
- **ubuntu-24.04-arm**: Linux aarch64, aarch64, 2 logical CPUs, SIMD asimd, sve, sve2; numpy 2.4.6, pennylane 0.45.1, pennylane-lightning 0.45.0, qiskit 2.6.2, qiskit-aer 0.18.4, vikshep-quantum 0.1.0
- **macos-15**: Darwin arm64, Apple M2 Pro (Virtual), 5 logical CPUs, SIMD n/a; numpy 2.4.6, pennylane 0.45.1, pennylane-lightning 0.45.0, qiskit 2.6.2, qiskit-aer 0.18.4, vikshep-quantum 0.1.0
- **windows-latest**: Windows AMD64, AMD64 Family 25 Model 1 Stepping 1, AuthenticAMD, 2 logical CPUs, SIMD n/a; numpy 2.4.6, pennylane 0.45.1, pennylane-lightning 0.45.0, qiskit 2.6.2, qiskit-aer 0.18.4, vikshep-quantum 0.1.0
- **linux-x86_64-docker**: Linux x86_64, Intel(R) Core(TM) i7-7700HQ CPU @ 2.80GHz, 8 logical CPUs, SIMD avx2, fma; numpy 2.4.6, pennylane 0.45.1, pennylane-lightning 0.45.0, qiskit 2.6.2, qiskit-aer 0.18.4, vikshep-quantum 0.1.0
- **macos-x86_64**: Darwin x86_64, Intel(R) Core(TM) i7-7700HQ CPU @ 2.80GHz, 8 logical CPUs, SIMD n/a; numpy 2.4.6, qiskit 2.6.2, qiskit-aer 0.18.4, vikshep-quantum 0.1.0

## ubuntu-latest: each setting against the default setting

Default setting: all threads, fusion on (Aer), default kernel (Lightning). `=` means identical bytes; otherwise the largest absolute difference. Shots: of 1,000,000 with the same seed, how many differ.

| Setting | qft20 state | qft20 expect. | qft20 shots | random16 state | random16 expect. | random16 shots | random20 state | random20 expect. | random20 shots | training |
|---|---|---|---|---|---|---|---|---|---|---|
| `aer-double-t2-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-double-t1-fusion` | = | 5.9e-14 | 0 | = | 1.2e-15 | 0 | = | 3.0e-15 | 0 | identical |
| `aer-single-t2-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-single-t1-fusion` | = | 1.1e-14 | 0 | = | = | 0 | = | 2.2e-16 | 0 | identical |
| `lightning-double-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-double-t1` | = | 1.1e-16 | 0 | = | 9.7e-17 | 0 | = | 2.2e-19 | 0 | identical |
| `lightning-single-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-single-t1` | = | 9.5e-07 | 0 | = | 3.0e-08 | 0 | = | 6.0e-08 | 0 | identical |
| `vikshep-single-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `vikshep-single-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |

Agreement between simulators (largest probability difference against `aer-double-t2-fusion`): qft20: aer-single-t2-fusion 5.0e-09, lightning-double-t2 2.6e-17, lightning-single-t2 7.3e-09, vikshep-single-t2 5.6e-09; random16: aer-single-t2-fusion 2.2e-10, lightning-double-t2 5.7e-19, lightning-single-t2 4.5e-10, vikshep-single-t2 2.3e-10; random20: aer-single-t2-fusion 2.9e-11, lightning-double-t2 1.2e-19, lightning-single-t2 6.3e-11, vikshep-single-t2 6.5e-11

## ubuntu-24.04-arm: each setting against the default setting

Default setting: all threads, fusion on (Aer), default kernel (Lightning). `=` means identical bytes; otherwise the largest absolute difference. Shots: of 1,000,000 with the same seed, how many differ.

| Setting | qft20 state | qft20 expect. | qft20 shots | random16 state | random16 expect. | random16 shots | random20 state | random20 expect. | random20 shots | training |
|---|---|---|---|---|---|---|---|---|---|---|
| `aer-double-t2-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-double-t1-fusion` | 7.8e-18 | 6.2e-14 | 0 | = | 8.9e-16 | 0 | = | 3.2e-15 | 0 | identical |
| `aer-single-t2-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-single-t1-fusion` | 9.3e-10 | 4.7e-10 | 26 | = | = | 0 | = | 2.2e-16 | 0 | identical |
| `lightning-double-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-double-t1` | = | 1.8e-15 | 0 | = | 8.3e-17 | 0 | = | 5.6e-17 | 0 | identical |
| `lightning-single-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-single-t1` | = | 1.9e-09 | 0 | = | 3.7e-08 | 0 | = | 3.0e-08 | 0 | identical |
| `vikshep-single-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `vikshep-single-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |

Agreement between simulators (largest probability difference against `aer-double-t2-fusion`): qft20: aer-single-t2-fusion 5.0e-09, lightning-double-t2 2.6e-17, lightning-single-t2 7.3e-09, vikshep-single-t2 5.6e-09; random16: aer-single-t2-fusion 2.0e-10, lightning-double-t2 4.9e-19, lightning-single-t2 5.8e-10, vikshep-single-t2 2.3e-10; random20: aer-single-t2-fusion 2.4e-11, lightning-double-t2 9.1e-20, lightning-single-t2 7.2e-11, vikshep-single-t2 6.5e-11

## macos-15: each setting against the default setting

Default setting: all threads, fusion on (Aer), default kernel (Lightning). `=` means identical bytes; otherwise the largest absolute difference. Shots: of 1,000,000 with the same seed, how many differ.

| Setting | qft20 state | qft20 expect. | qft20 shots | random16 state | random16 expect. | random16 shots | random20 state | random20 expect. | random20 shots | training |
|---|---|---|---|---|---|---|---|---|---|---|
| `aer-double-t5-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-double-t1-fusion` | = | 2.0e-13 | 0 | = | 2.4e-15 | 0 | = | 1.8e-14 | 0 | identical |
| `aer-single-t5-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-single-t1-fusion` | = | 7.1e-15 | 0 | = | 2.8e-17 | 0 | = | 6.7e-16 | 0 | identical |
| `lightning-double-t5` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-double-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |
| `lightning-single-t5` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-single-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |
| `vikshep-single-t5` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `vikshep-single-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |

Agreement between simulators (largest probability difference against `aer-double-t5-fusion`): qft20: aer-single-t5-fusion 5.0e-09, lightning-double-t5 2.6e-17, lightning-single-t5 6.4e-09, vikshep-single-t5 5.6e-09; random16: aer-single-t5-fusion 2.2e-10, lightning-double-t5 6.2e-19, lightning-single-t5 5.3e-10, vikshep-single-t5 2.3e-10; random20: aer-single-t5-fusion 2.9e-11, lightning-double-t5 1.2e-19, lightning-single-t5 9.0e-11, vikshep-single-t5 6.5e-11

## windows-latest: each setting against the default setting

Default setting: all threads, fusion on (Aer), default kernel (Lightning). `=` means identical bytes; otherwise the largest absolute difference. Shots: of 1,000,000 with the same seed, how many differ.

| Setting | qft20 state | qft20 expect. | qft20 shots | random16 state | random16 expect. | random16 shots | random20 state | random20 expect. | random20 shots | training |
|---|---|---|---|---|---|---|---|---|---|---|
| `aer-double-t2-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-double-t1-fusion` | = | 5.9e-14 | 0 | = | 5.1e-16 | 0 | = | 3.1e-15 | 0 | identical |
| `aer-single-t2-fusion` | failed (exit code 3221225477) |  |  |  |  |  |  |  |  |  |
| `aer-single-t1-fusion` | failed (exit code 3221225477) |  |  |  |  |  |  |  |  |  |
| `lightning-double-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-double-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |
| `lightning-single-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-single-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |
| `vikshep-single-t2` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `vikshep-single-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |

Agreement between simulators (largest probability difference against `aer-double-t2-fusion`): qft20: lightning-double-t2 4.8e-17, lightning-single-t2 6.4e-09, vikshep-single-t2 5.6e-09; random16: lightning-double-t2 5.7e-19, lightning-single-t2 5.0e-10, vikshep-single-t2 2.3e-10; random20: lightning-double-t2 8.1e-20, lightning-single-t2 9.3e-11, vikshep-single-t2 6.5e-11

## linux-x86_64-docker: each setting against the default setting

Default setting: all threads, fusion on (Aer), default kernel (Lightning). `=` means identical bytes; otherwise the largest absolute difference. Shots: of 1,000,000 with the same seed, how many differ.

| Setting | qft20 state | qft20 expect. | qft20 shots | random16 state | random16 expect. | random16 shots | random20 state | random20 expect. | random20 shots | training |
|---|---|---|---|---|---|---|---|---|---|---|
| `aer-double-t8-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-double-t4-fusion` | = | 7.6e-14 | 0 | = | 2.7e-15 | 0 | = | 1.2e-14 | 0 | identical |
| `aer-double-t2-fusion` | = | 1.6e-13 | 0 | = | 3.0e-15 | 0 | = | 2.9e-14 | 0 | identical |
| `aer-double-t1-fusion` | = | 2.2e-13 | 0 | = | 1.8e-15 | 0 | = | 3.1e-14 | 0 | identical |
| `aer-double-t8-nofusion` | 2.5e-17 | 2.1e-17 | 0 | 2.6e-17 | 2.8e-16 | 0 | 1.1e-17 | 8.3e-16 | 0 | identical |
| `aer-double-t4-nofusion` | 2.5e-17 | 7.6e-14 | 0 | 2.6e-17 | 3.1e-15 | 0 | 1.1e-17 | 1.3e-14 | 0 | identical |
| `aer-double-t2-nofusion` | 2.5e-17 | 1.6e-13 | 0 | 2.6e-17 | 3.2e-15 | 0 | 1.1e-17 | 3.0e-14 | 0 | identical |
| `aer-double-t1-nofusion` | 2.5e-17 | 2.2e-13 | 0 | 2.6e-17 | 1.9e-15 | 0 | 1.1e-17 | 3.2e-14 | 0 | identical |
| `aer-single-t8-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-single-t4-fusion` | = | 5.3e-15 | 0 | = | 2.8e-17 | 0 | = | = | 0 | identical |
| `aer-single-t2-fusion` | = | 1.8e-15 | 0 | = | = | 0 | = | 4.4e-16 | 0 | identical |
| `aer-single-t1-fusion` | = | 8.9e-15 | 0 | = | = | 0 | = | 6.7e-16 | 0 | identical |
| `aer-single-t8-nofusion` | 2.6e-08 | 6.0e-06 | 95694 | 1.6e-08 | 1.8e-07 | 22268 | 6.1e-09 | 4.8e-07 | 271640 | differs from step 0 (max param diff 1.5e-01) |
| `aer-single-t4-nofusion` | 2.6e-08 | 6.0e-06 | 95694 | 1.6e-08 | 1.8e-07 | 22268 | 6.1e-09 | 4.8e-07 | 271640 | differs from step 0 (max param diff 1.5e-01) |
| `aer-single-t2-nofusion` | 2.6e-08 | 6.0e-06 | 95694 | 1.6e-08 | 1.8e-07 | 22268 | 6.1e-09 | 4.8e-07 | 271640 | differs from step 0 (max param diff 1.5e-01) |
| `aer-single-t1-nofusion` | 2.6e-08 | 6.0e-06 | 95694 | 1.6e-08 | 1.8e-07 | 22268 | 6.1e-09 | 4.8e-07 | 271640 | differs from step 0 (max param diff 1.5e-01) |
| `lightning-double-t8` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-double-t4` | = | = | 0 | = | 4.2e-17 | 0 | = | 5.6e-17 | 0 | identical |
| `lightning-double-t2` | = | 1.8e-15 | 0 | = | 4.2e-17 | 0 | = | 5.6e-17 | 0 | identical |
| `lightning-double-t1` | = | 3.6e-15 | 0 | = | 1.4e-16 | 0 | = | 5.6e-17 | 0 | identical |
| `lightning-double-t8-LM` | = | 1.8e-15 | 0 | = | 2.8e-17 | 0 | = | 5.6e-17 | 0 | identical |
| `lightning-double-t8-AVX2` | = | 1.8e-15 | 0 | = | 1.1e-16 | 0 | = | 2.8e-17 | 0 | identical |
| `lightning-double-t8-AVX512` | = | 1.8e-15 | 0 | = | 1.4e-17 | 0 | = | 1.1e-19 | 0 | identical |
| `lightning-single-t8` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `lightning-single-t4` | = | 1.9e-06 | 0 | = | = | 0 | = | 6.0e-08 | 0 | identical |
| `lightning-single-t2` | = | 9.5e-07 | 0 | = | 3.0e-08 | 0 | = | 3.0e-08 | 0 | identical |
| `lightning-single-t1` | = | = | 0 | = | 6.0e-08 | 0 | = | 1.5e-08 | 0 | identical |
| `lightning-single-t8-LM` | = | 9.5e-07 | 0 | = | = | 0 | = | 6.0e-08 | 0 | identical |
| `lightning-single-t8-AVX2` | = | 9.5e-07 | 0 | = | = | 0 | = | 1.2e-10 | 0 | identical |
| `lightning-single-t8-AVX512` | = | 9.5e-07 | 0 | = | 6.0e-08 | 0 | = | 3.0e-08 | 0 | identical |
| `vikshep-single-t8` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `vikshep-single-t4` | = | = | 0 | = | = | 0 | = | = | 0 | identical |
| `vikshep-single-t2` | = | = | 0 | = | = | 0 | = | = | 0 | identical |
| `vikshep-single-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |

Agreement between simulators (largest probability difference against `aer-double-t8-fusion`): qft20: aer-single-t8-fusion 5.0e-09, lightning-double-t8 2.6e-17, lightning-single-t8 7.3e-09, vikshep-single-t8 5.6e-09; random16: aer-single-t8-fusion 2.2e-10, lightning-double-t8 5.7e-19, lightning-single-t8 4.5e-10, vikshep-single-t8 2.3e-10; random20: aer-single-t8-fusion 2.9e-11, lightning-double-t8 1.2e-19, lightning-single-t8 6.3e-11, vikshep-single-t8 6.5e-11

## macos-x86_64: each setting against the default setting

Default setting: all threads, fusion on (Aer), default kernel (Lightning). `=` means identical bytes; otherwise the largest absolute difference. Shots: of 1,000,000 with the same seed, how many differ.

| Setting | qft20 state | qft20 expect. | qft20 shots | random16 state | random16 expect. | random16 shots | random20 state | random20 expect. | random20 shots | training |
|---|---|---|---|---|---|---|---|---|---|---|
| `aer-double-t8-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-double-t4-fusion` | = | 7.6e-14 | 0 | = | 3.0e-15 | 0 | = | 1.2e-14 | 0 | identical |
| `aer-double-t2-fusion` | = | 1.6e-13 | 0 | = | 3.1e-15 | 0 | = | 2.9e-14 | 0 | identical |
| `aer-double-t1-fusion` | = | 2.3e-13 | 0 | = | 2.1e-15 | 0 | = | 3.2e-14 | 0 | identical |
| `aer-double-t8-nofusion` | 2.1e-17 | 3.6e-15 | 0 | 2.7e-17 | 2.8e-16 | 0 | 9.5e-18 | 3.9e-16 | 0 | identical |
| `aer-double-t4-nofusion` | 2.1e-17 | 7.6e-14 | 0 | 2.7e-17 | 2.8e-15 | 0 | 9.5e-18 | 1.2e-14 | 0 | identical |
| `aer-double-t2-nofusion` | 2.1e-17 | 1.7e-13 | 0 | 2.7e-17 | 2.8e-15 | 0 | 9.5e-18 | 2.9e-14 | 0 | identical |
| `aer-double-t1-nofusion` | 2.1e-17 | 2.3e-13 | 0 | 2.7e-17 | 1.9e-15 | 0 | 9.5e-18 | 3.2e-14 | 0 | identical |
| `aer-single-t8-fusion` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `aer-single-t4-fusion` | = | 5.3e-15 | 0 | = | = | 0 | = | 4.4e-16 | 0 | identical |
| `aer-single-t2-fusion` | = | 1.8e-15 | 0 | = | 2.8e-17 | 0 | = | 6.7e-16 | 0 | identical |
| `aer-single-t1-fusion` | = | 8.9e-15 | 0 | = | 2.8e-17 | 0 | = | 8.9e-16 | 0 | identical |
| `aer-single-t8-nofusion` | 2.6e-08 | 6.0e-06 | 95694 | 1.6e-08 | 1.8e-07 | 22268 | 6.1e-09 | 4.8e-07 | 271640 | differs from step 0 (max param diff 1.5e-01) |
| `aer-single-t4-nofusion` | 2.6e-08 | 6.0e-06 | 95694 | 1.6e-08 | 1.8e-07 | 22268 | 6.1e-09 | 4.8e-07 | 271640 | differs from step 0 (max param diff 1.5e-01) |
| `aer-single-t2-nofusion` | 2.6e-08 | 6.0e-06 | 95694 | 1.6e-08 | 1.8e-07 | 22268 | 6.1e-09 | 4.8e-07 | 271640 | differs from step 0 (max param diff 1.5e-01) |
| `aer-single-t1-nofusion` | 2.6e-08 | 6.0e-06 | 95694 | 1.6e-08 | 1.8e-07 | 22268 | 6.1e-09 | 4.8e-07 | 271640 | differs from step 0 (max param diff 1.5e-01) |
| `vikshep-single-t8` | (default) |  |  | (default) |  |  | (default) |  |  | (default) |
| `vikshep-single-t4` | = | = | 0 | = | = | 0 | = | = | 0 | identical |
| `vikshep-single-t2` | = | = | 0 | = | = | 0 | = | = | 0 | identical |
| `vikshep-single-t1` | = | = | 0 | = | = | 0 | = | = | 0 | identical |

Agreement between simulators (largest probability difference against `aer-double-t8-fusion`): qft20: aer-single-t8-fusion 5.0e-09, vikshep-single-t8 5.6e-09; random16: aer-single-t8-fusion 2.2e-10, vikshep-single-t8 2.3e-10; random20: aer-single-t8-fusion 2.9e-11, vikshep-single-t8 6.5e-11

## Across machines

Settings present on every machine (`tall` is each machine's own thread count, the default). For each, are the bytes the same on every machine? "differs" lists the circuits that differ.

| Setting | state | probabilities | shots | expectations (max diff) | training |
|---|---|---|---|---|---|
| `aer-double-tall-fusion` | differs (qft20, random16, random20) | differs (qft20, random16, random20) | same | 1.6e-13 | identical |
| `aer-double-t1-fusion` | differs (qft20, random16, random20) | differs (qft20, random16, random20) | same | 3.9e-14 | identical |
| `aer-single-tall-fusion` | failed on windows-latest | | | | |
| `aer-single-t1-fusion` | failed on windows-latest | | | | |
| `vikshep-single-tall` | same | same | same | 0 | identical |
| `vikshep-single-t1` | same | same | same | 0 | identical |


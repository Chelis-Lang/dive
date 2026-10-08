# Standard modules

These are SANY's standard modules: the `.tla` files of `tla2sany/StandardModules/` in the
pinned `tla2tools.jar` (tlaplus `94d0c50`, sha256 `7beec0f0…ea78d`). They are copied byte
for byte and are identical to the same directory of the tlaplus source tree at that
commit. `src/stdlib.rs` embeds them; ADR 0011 explains why they live in this crate.

**Licence.** MIT, © HP Corporation, Microsoft Corporation and the Linux Foundation; see
`LICENSE`, which is tlaplus's licence file.

**Updating.** When the SANY pin moves:
1. extract `tla2sany/StandardModules/*.tla` from the new jar;
2. replace these files;
3. update the table of digests below;
4. review the diff.

Then re-run the acceptance procedure in `../acceptance/README.md`.

**Digests (sha256):**

| File | sha256 |
|---|---|
| `Bags.tla` | `a2b668a8cf375b9ef9f2c14fdba742137ab8722ade0f41f11965c67acba76c39` |
| `FiniteSets.tla` | `d94cb545256539bf782fc1914842dfe8c19c1d3d4e48b8327dd0c47b74922e1e` |
| `Integers.tla` | `071da69e5eea383d361ef308ba12ecc931bf968fcea98175c3bca419bbd8cbf2` |
| `Json.tla` | `52e38eeed89c35ef9d65ac7f6e884a8ae761c7302a52c35c0f0658035ddca0b8` |
| `Naturals.tla` | `73f2a68412039aa86f37ad08f0d0fef50fa8327d0ad8806f515548c75e9d9d84` |
| `Randomization.tla` | `998209fcb33bac3c9b4d6851dc5df5ca83d33722201cc5280b51a1f58fffaebf` |
| `RealTime.tla` | `9052ccc8a150c93ae13a4d136da7b83dc68d1d8e9a0eee6f787db02b755f6682` |
| `Reals.tla` | `33cc699c0139b39fcee903d4dde255b64a0b430291630aa4825c6dd32b2f2fa2` |
| `Sequences.tla` | `f613cffe14133769f0972b6f6e0139aaf0773004a51bac88ada382426e3eb7f4` |
| `TLC.tla` | `944ece590eec81ace059881c2a62064c06041efa81539a788cd40fa7d1da41ce` |
| `TLCExt.tla` | `6c3e7e54cf1b8a4d2d1c7a5fd088f06452edce68ec28d8be99392cc0037a5399` |
| `Toolbox.tla` | `9deac1facb0b71809714229dace116da75f80bf4c65b707ff40026f380c7804a` |
| `_DotTrace.tla` | `c2f4b13446298c026decf0b3867134d2a176355344db05af7270e31981f0c8bc` |
| `_JsonTrace.tla` | `4e93f1de053afb59cbeb5ae6f0d71685acf3530c840ac4811f54e86100195742` |
| `_Possible.tla` | `3c360af74043d85a0399376aa2b1553ca9d230d06a34bd46cbae1f1e02109a13` |
| `_TLAPlusCounterExample.tla` | `ebc408fd614b832d2f8eab1a4dae7ae67c6d5a7958e0233c7da702c1a0e29705` |
| `_TLCActionTrace.tla` | `4cb2f1e02109cee0053f59e72bdfb59edab12a30cddf4bc1765927288db2d134` |
| `_TLCTESpec.tla` | `ee5b74e29dcd8d64f2a8840c293018a2007371aab14ea0eaa7c542de8f97b4aa` |
| `_TLCTrace.tla` | `864407a426e7e438af3652a5db92912bfdf682cf97912c6bdc91d87cb94a5007` |
| `_TLCTracePlain.tla` | `4f05472f833eadec96a2ec8cb2f1331ce8902bf08c800b80407d2980809e6d09` |

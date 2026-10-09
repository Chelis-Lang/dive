# Direct SANY grammar driver

`DiveSanyParse` calls the parser that `ParseUnit.parseFile` uses at the pinned
TLC commit `94d0c5070852b37b801bb8998c436383ab7ee37b`. It supplies UTF-8 bytes
and silent output, records `parse()`'s boolean, and checks the resulting module
name against the filename. It does not load modules or validate PlusCal.
Unexpected exceptions exit nonzero and are oracle errors, never rejections.

The driver is original MIT code. The checked bytecode avoids requiring a system
JDK for campaigns; it executes on the pinned Temurin JRE. Rebuild with Temurin
17.0.20.1+1:

```sh
javac --release 17 -g:none \
  -cp "$DIVE_CACHE_DIR/sha256/7beec0f04818732a62fa193731711a99aa4f11279499b2360a7d156c519ea78d/tla2tools.jar" \
  -d scripts/java scripts/java/DiveSanyParse.java
```

Source SHA-256: `012167ef75cbce6bbfade559ad3bd9b7d577ea2401ee853b680f507bf83fbde5`.
Bytecode SHA-256: `9cdcf84346808f0079e7fdb7ca7cc2eb411383b4feb71bcaa4df8abe48baed50`.
The runner verifies both before execution, and reports both pins alongside the
oracle provenance. Changing the driver requires rebuilding and updating both
pins with positive and negative runtime controls.

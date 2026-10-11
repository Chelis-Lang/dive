# Failed launcher preparation

The copied archival launcher requires a precreated `raw/` directory. Its parent
was omitted, so every attempted model wrapper failed before `Popen` executed the
native binary. The full selection/report is retained unchanged: 140 selected,
138 unclassified launcher disagreements, two harness-side refusals, actual
harness exit 1 and zero native subprocess invocations. Source and executable
hashes stayed unchanged. These rows earn no semantic agreement/disagreement
credit; no reported outcome was rewritten. A fresh complete run precreates
`raw/` and uses the unchanged launcher.

---- MODULE OpSubMid ----
CONSTANTS F(_), A(_)
VARIABLE x
Inner == INSTANCE OpSubInner WITH G <- F, Act <- A
====

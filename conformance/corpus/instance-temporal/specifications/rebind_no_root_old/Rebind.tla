---------------- MODULE Rebind ----------------
EXTENDS Pair
Fresh == INSTANCE Pair WITH y <- TRUE, z <- FALSE
CanOld == ENABLED A
CanFresh == ENABLED Fresh!A
================================================

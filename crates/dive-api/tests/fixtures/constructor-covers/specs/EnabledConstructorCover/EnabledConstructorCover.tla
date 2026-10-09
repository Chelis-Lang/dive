---- MODULE EnabledConstructorCover ----
VARIABLES S,t
Init == S \in SUBSET BOOLEAN /\ t \in BOOLEAN
Next == UNCHANGED S /\ t'=~t
Map == UNCHANGED S /\ t' \in BOOLEAN /\ {t'=b : b \in BOOLEAN}={TRUE,FALSE}
Filter == UNCHANGED S /\ t' \in BOOLEAN /\ {b \in BOOLEAN : b=t'}={TRUE}
Function == UNCHANGED S /\ t' \in BOOLEAN /\ [b \in BOOLEAN |-> b=t'][TRUE]
Nested == UNCHANGED S /\ t' \in BOOLEAN /\ TRUE \in [b \in BOOLEAN |-> {b=c /\ t'=c : c \in BOOLEAN}][TRUE]
Current == UNCHANGED S /\ t' \in BOOLEAN /\ {b \in S : b=t'}={TRUE}
Empty == UNCHANGED S /\ {t'=b : b \in {}}={}
I == ENABLED Map /\ ENABLED Filter /\ ENABLED Function /\ ENABLED Nested /\ ENABLED Empty /\ ((ENABLED Current) = (TRUE \in S))
====

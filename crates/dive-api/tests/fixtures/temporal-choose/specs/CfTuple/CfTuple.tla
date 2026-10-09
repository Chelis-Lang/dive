---- MODULE CfTuple ----
EXTENDS Naturals
VARIABLE x
Init == x=FALSE
Next == x'=~x
P == []<>((CHOOSE t \in IF x THEN {<<TRUE,FALSE>>} ELSE {<<FALSE,TRUE>>} : t[1]#t[2]) = <<TRUE,FALSE>>)
====

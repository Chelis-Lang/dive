---- MODULE RndConfigured ----
EXTENDS Naturals,TLC
CONSTANT C
VARIABLE x
Pick==RandomElement(1..1000000)
Init==x=C
Next==x'=C
Safe==TRUE
====

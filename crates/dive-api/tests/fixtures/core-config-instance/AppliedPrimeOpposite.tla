---- MODULE AppliedPrimeOpposite ----
EXTENDS Naturals
CONSTANT C(_)
VARIABLE x
Target(n)==n=n
Init==x=0
Next==x'=IF x=0 THEN 1 ELSE x
Prop==[][~C(x')]_x
====

---- MODULE SharedDag20 ----
EXTENDS Naturals
VARIABLES x,p
Init == x = 0 /\ p = 0
Next == /\ p = 0
        /\ p' = 1
        /\ x' = LET a0 == x
           a1 == <<a0, a0>>
           a2 == <<a1, a1>>
           a3 == <<a2, a2>>
           a4 == <<a3, a3>>
           a5 == <<a4, a4>>
           a6 == <<a5, a5>>
           a7 == <<a6, a6>>
           a8 == <<a7, a7>>
           a9 == <<a8, a8>>
           a10 == <<a9, a9>>
           a11 == <<a10, a10>>
           a12 == <<a11, a11>>
           a13 == <<a12, a12>>
           a14 == <<a13, a13>>
           a15 == <<a14, a14>>
           a16 == <<a15, a15>>
           a17 == <<a16, a16>>
           a18 == <<a17, a17>>
           a19 == <<a18, a18>>
           a20 == <<a19, a19>>
                 IN a20
====

---- MODULE Pc_C_procedure ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A { procedure P(a = 0, b) variable z = 1; { R: return; } { L: call P(1, 2); N: skip; } }

*)
\* BEGIN TRANSLATION (chksum(pcal) = "a253ed0e" /\ chksum(tla) = "25dbe0fe")
CONSTANT defaultInitValue
VARIABLES pc, stack, a, b, z

vars == << pc, stack, a, b, z >>

Init == (* Procedure P *)
        /\ a = 0
        /\ b = defaultInitValue
        /\ z = 1
        /\ stack = << >>
        /\ pc = "L"

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ z' = Head(stack).z
     /\ a' = Head(stack).a
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)

P == R

L == /\ pc = "L"
     /\ /\ a' = 1
        /\ b' = 2
        /\ stack' = << [ procedure |->  "P",
                         pc        |->  "N",
                         z         |->  z,
                         a         |->  a,
                         b         |->  b ] >>
                     \o stack
     /\ z' = 1
     /\ pc' = "R"

N == /\ pc = "N"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << stack, a, b, z >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == P \/ L \/ N
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====

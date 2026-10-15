---- MODULE ProcedureGraph ----
EXTENDS Integers, TLC
CONSTANT S
(*
--algorithm A variable x=0; procedure Outer(a=0) variable u=1; begin O: call Inner(a+u); R: return; end procedure; procedure Inner(b=0) variable v=2; begin I: x:=b+v; S: return; end procedure; begin L: call Outer(3); E: skip; end algorithm
*)
\* BEGIN TRANSLATION (chksum(pcal) = "44efb2e4" /\ chksum(tla) = "e532b2ae")
VARIABLES pc, x, stack, a, u, b, v

vars == << pc, x, stack, a, u, b, v >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Outer *)
        /\ a = 0
        /\ u = 1
        (* Procedure Inner *)
        /\ b = 0
        /\ v = 2
        /\ stack = << >>
        /\ pc = "L"

O == /\ pc = "O"
     /\ /\ b' = a+u
        /\ stack' = << [ procedure |->  "Inner",
                         pc        |->  "R",
                         v         |->  v,
                         b         |->  b ] >>
                     \o stack
     /\ v' = 2
     /\ pc' = "I"
     /\ UNCHANGED << x, a, u >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ u' = Head(stack).u
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, v >>

Outer == O \/ R

I == /\ pc = "I"
     /\ x' = b+v
     /\ pc' = "S"
     /\ UNCHANGED << stack, a, u, b, v >>

S == /\ pc = "S"
     /\ pc' = Head(stack).pc
     /\ v' = Head(stack).v
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, u >>

Inner == I \/ S

L == /\ pc = "L"
     /\ /\ a' = 3
        /\ stack' = << [ procedure |->  "Outer",
                         pc        |->  "E",
                         u         |->  u,
                         a         |->  a ] >>
                     \o stack
     /\ u' = 1
     /\ pc' = "O"
     /\ UNCHANGED << x, b, v >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, u, b, v >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Outer \/ Inner \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====

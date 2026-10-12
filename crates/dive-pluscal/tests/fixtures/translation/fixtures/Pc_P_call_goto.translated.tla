---- MODULE Pc_P_call_goto ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A procedure P() begin R: return; end procedure; begin L: call P(); goto "Done"; end algorithm

*)
\* BEGIN TRANSLATION (chksum(pcal) = "46b058e6" /\ chksum(tla) = "8cba269e")
VARIABLES pc, stack

vars == << pc, stack >>

Init == /\ stack = << >>
        /\ pc = "L"

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ stack' = Tail(stack)

P == R

L == /\ pc = "L"
     /\ stack' = << [ procedure |->  "P",
                      pc        |->  "Done" ] >>
                  \o stack
     /\ pc' = "R"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == P \/ L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====

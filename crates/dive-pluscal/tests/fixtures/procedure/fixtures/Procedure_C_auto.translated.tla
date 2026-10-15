---- MODULE Procedure_C_auto ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
x:=a; return;
};
{
call Put(2); skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "89fdfcc4" /\ chksum(tla) = "815b86b2")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "Lbl_2"

Lbl_1 == /\ pc = "Lbl_1"
         /\ x' = a
         /\ pc' = Head(stack).pc
         /\ a' = Head(stack).a
         /\ stack' = Tail(stack)

Put == Lbl_1

Lbl_2 == /\ pc = "Lbl_2"
         /\ /\ a' = 2
            /\ stack' = << [ procedure |->  "Put",
                             pc        |->  "Lbl_3",
                             a         |->  a ] >>
                         \o stack
         /\ pc' = "Lbl_1"
         /\ x' = x

Lbl_3 == /\ pc = "Lbl_3"
         /\ TRUE
         /\ pc' = "Done"
         /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ Lbl_2 \/ Lbl_3
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====

---- MODULE ProcedureGraph_P_auto_labels ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
call Second(a); return;
end procedure;
procedure Second(b=1)

begin
x:=b; return;
end procedure;
begin
call First(2); skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "69f10d7c" /\ chksum(tla) = "dbcdc29a")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 1
        /\ stack = << >>
        /\ pc = "Lbl_3"

Lbl_1 == /\ pc = "Lbl_1"
         /\ /\ b' = a
            /\ stack' = << [ procedure |->  "Second",
                             pc        |->  Head(stack).pc,
                             b         |->  b ] >>
                         \o Tail(stack)
         /\ pc' = "Lbl_2"
         /\ UNCHANGED << x, a >>

First == Lbl_1

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = b
         /\ pc' = Head(stack).pc
         /\ b' = Head(stack).b
         /\ stack' = Tail(stack)
         /\ a' = a

Second == Lbl_2

Lbl_3 == /\ pc = "Lbl_3"
         /\ /\ a' = 2
            /\ stack' = << [ procedure |->  "First",
                             pc        |->  "Lbl_4",
                             a         |->  a ] >>
                         \o stack
         /\ pc' = "Lbl_1"
         /\ UNCHANGED << x, b >>

Lbl_4 == /\ pc = "Lbl_4"
         /\ TRUE
         /\ pc' = "Done"
         /\ UNCHANGED << x, stack, a, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ Lbl_3 \/ Lbl_4
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====

---- MODULE Recursive_C_auto_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
if(a>0){ call First(a-1); }else{ x:=a; }; return;
};
{
call First(2); skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "92002444" /\ chksum(tla) = "79cc55a6")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "Lbl_3"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF a>0
               THEN /\ /\ a' = a-1
                       /\ stack' = << [ procedure |->  "First",
                                        pc        |->  "Lbl_2",
                                        a         |->  a ] >>
                                    \o stack
                    /\ pc' = "Lbl_1"
                    /\ x' = x
               ELSE /\ x' = a
                    /\ pc' = "Lbl_2"
                    /\ UNCHANGED << stack, a >>

Lbl_2 == /\ pc = "Lbl_2"
         /\ pc' = Head(stack).pc
         /\ a' = Head(stack).a
         /\ stack' = Tail(stack)
         /\ x' = x

First == Lbl_1 \/ Lbl_2

Lbl_3 == /\ pc = "Lbl_3"
         /\ /\ a' = 2
            /\ stack' = << [ procedure |->  "First",
                             pc        |->  "Lbl_4",
                             a         |->  a ] >>
                         \o stack
         /\ pc' = "Lbl_1"
         /\ x' = x

Lbl_4 == /\ pc = "Lbl_4"
         /\ TRUE
         /\ pc' = "Done"
         /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Lbl_3 \/ Lbl_4
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====

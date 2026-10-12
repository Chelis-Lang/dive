---- MODULE ProcedureGraph_P_four_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=1)

begin
S:call Third(b); T:return;
end procedure;
procedure Third(c=0)

begin
U:call Fourth(c); V:return;
end procedure;
procedure Fourth(d=0)

begin
W:x:=d; Q:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "4be89388" /\ chksum(tla) = "60aa84")
VARIABLES pc, x, stack, a, b, c, d

vars == << pc, x, stack, a, b, c, d >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 1
        (* Procedure Third *)
        /\ c = 0
        (* Procedure Fourth *)
        /\ d = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "R",
                         b         |->  b ] >>
                     \o stack
     /\ pc' = "S"
     /\ UNCHANGED << x, a, c, d >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, c, d >>

First == F \/ R

S == /\ pc = "S"
     /\ /\ c' = b
        /\ stack' = << [ procedure |->  "Third",
                         pc        |->  "T",
                         c         |->  c ] >>
                     \o stack
     /\ pc' = "U"
     /\ UNCHANGED << x, a, b, d >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, c, d >>

Second == S \/ T

U == /\ pc = "U"
     /\ /\ d' = c
        /\ stack' = << [ procedure |->  "Fourth",
                         pc        |->  "V",
                         d         |->  d ] >>
                     \o stack
     /\ pc' = "W"
     /\ UNCHANGED << x, a, b, c >>

V == /\ pc = "V"
     /\ pc' = Head(stack).pc
     /\ c' = Head(stack).c
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, b, d >>

Third == U \/ V

W == /\ pc = "W"
     /\ x' = d
     /\ pc' = "Q"
     /\ UNCHANGED << stack, a, b, c, d >>

Q == /\ pc = "Q"
     /\ pc' = Head(stack).pc
     /\ d' = Head(stack).d
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, b, c >>

Fourth == W \/ Q

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b, c, d >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b, c, d >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ Third \/ Fourth \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====

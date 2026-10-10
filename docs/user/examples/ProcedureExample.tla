---- MODULE ProcedureExample ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Put(a = 0)
variable b = a + 1;
begin
  Entry: x := b;
  Return: return;
end procedure;
begin
  Start: call Put(2);
  Finish: skip;
end algorithm; *)
====

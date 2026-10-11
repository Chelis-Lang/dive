---- MODULE FairCall ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x = 0;
procedure Add(a = 0)
begin
  Entry: + x := x + a;
  Finish: return;
end procedure;
fair process One = 1
begin
  Start: call Add(1);
  DoneOne: skip;
end process;
end algorithm; *)
====

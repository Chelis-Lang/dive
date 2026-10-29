---- MODULE Representation ----
EXTENDS Naturals
VARIABLE x
Init == x = [k \in {"a"} |-> 0]
Next == x' = [k \in {"a"} |-> 0] \/ x' = [a |-> 0]
ActionCheck == <>[][~(x' = <<0, 0>>)]_x
StateControl == <>[]~(x = <<0, 0>>)
Holds == []<>(x = [a |-> 0])
LiteralInit == x = [a |-> 0]
NestedGeneralInit == x = <<[k \in {"a"} |-> 0]>>
NestedLiteralInit == x = <<[a |-> 0]>>
NestedNext == x' = <<[k \in {"a"} |-> 0]>> \/ x' = <<[a |-> 0]>>
NestedStateControl == <>[]~(x = << <<0, 0>> >>)
====

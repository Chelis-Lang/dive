---- MODULE RvFcnRcd3 ----
EXTENDS Integers, TLC
ASSUME PrintT([x \in {"qa", "qb"} |-> 1] = <<1>>)
====

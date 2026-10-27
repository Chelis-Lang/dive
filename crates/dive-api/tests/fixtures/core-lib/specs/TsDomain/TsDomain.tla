---- MODULE TsDomain ----
\* DOMAIN of a sequence is TLC's interval, printed "1..2", and the core's
\* enumerated set: printing it is refused (crates/dive-core/src/print.rs).
EXTENDS Naturals, TLC
VARIABLE x
Init == x = <<1, 2>>
Next == UNCHANGED x
Inv == ToString(DOMAIN x) = "1..2"
====

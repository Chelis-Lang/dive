---- MODULE TraceValues ----
EXTENDS Integers, TLC
CONSTANTS mvq1, mvq2
VARIABLES qva, qvb, qvc
Init == /\ qva = [qfa |-> {1, 2}, qfb |-> <<"qsz", mvq1>>]
        /\ qvb = 1..3
        /\ qvc = "q<>&='\t\"\\"
Next == /\ qva' = [qva EXCEPT !.qfa = SUBSET {1, 2, 3}]
        /\ qvb' = (1 :> qvb @@ 2 :> {} @@ 3 :> (7 :> mvq2))
        /\ qvc' = <<{"qsz", "qsy"}, -5, TRUE>>
Inv == TLCGet("level") < 2
====

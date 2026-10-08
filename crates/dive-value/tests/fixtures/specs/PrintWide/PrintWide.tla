---- MODULE PrintWide ----
EXTENDS Integers, Sequences, FiniteSets, TLC, TLCExt
CONSTANTS mvq1, mvq2, T_q1, T_q2, U_q1, mv_q3
ASSUME PrintT("#bigset") /\ PrintT(SUBSET (1..5))
ASSUME PrintT("#bigrecs") /\ PrintT([qfa : {"qsu1", "qsu2", "qsu3"}, qfb : 1..4])
ASSUME PrintT("#bigfcn") /\ PrintT([{1, 2, 3} -> {"qsv1", "qsv2"}])
ASSUME PrintT("#bigfcn2") /\ PrintT((1 :> {"qsw1", "qsw2", "qsw3"} @@ 7 :> SUBSET {1, 2, 3} @@ 9 :> <<"qsw4", "qsw5">>))
ASSUME PrintT("#bignested") /\ PrintT([qfa |-> SUBSET {1, 2, 3}, qfb |-> [qfc |-> SUBSET {4, 5}, qfa |-> <<1, 2, 3>>]])
ASSUME PrintT("#bigunderscore") /\ PrintT(<<mv_q3, mv_q3, mv_q3, mv_q3, mv_q3, mv_q3, mv_q3, mv_q3, mv_q3, mv_q3, mv_q3, mv_q3, mv_q3>>)
ASSUME PrintT("#bigmvonly") /\ PrintT({mvq2, mvq1, <<mvq1, mvq2, mvq1, mvq2, mvq1, mvq2, mvq1, mvq2, mvq1, mvq2, mvq1>>})
ASSUME PrintT("#bigsetrecs") /\ PrintT({[qfa |-> 1, qfb |-> <<1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20>>], [qfa |-> 2, qfb |-> {}]})
ASSUME PrintT("#bigrecpair") /\ PrintT([qfa |-> <<1111111111, 1111111111, 1111111111, 1111111111, 1111111111, 1111111111, 1111111111>>])
ASSUME PrintT("#bigfcnkey") /\ PrintT((<<1111111111, 1111111111, 1111111111, 1111111111, 1111111111, 1111111111, 1111111111>> :> 1))
ASSUME PrintT("#bigsubsetlazy") /\ PrintT({SUBSET {1, 2, 3, 4, 5, 6, 7}, SUBSET {11, 12, 13, 14, 15, 16, 17}, SUBSET {21, 22, 23, 24, 25, 26, 27}})
ASSUME PrintT("#bigtuple") /\ PrintT(<<SUBSET {1, 2, 3}, SUBSET {4, 5, 6}>>)
ASSUME PrintT("#bigstrings") /\ PrintT({"qsx aaaaaaaaaaaaaaaaaaaa", "qsx bbbbbbbbbbbbbbbbbbbbbbbbbb", "qsx cccccccccccccccccccccccccccc", "qsx ddd"})
ASSUME PrintT("#bigquote") /\ PrintT(<<"q\"uote", 1111111111, 1111111111, 1111111111, 1111111111, 1111111111, 1111111111, 1111111111>>)
ASSUME PrintT("#bigintervals") /\ PrintT(<<1..1000, 1..1000, 1..1000, 1..1000, 1..1000, 1..1000, 1..1000, 1..1000, 1..1000>>)
ASSUME PrintT("#bignegint") /\ PrintT(<<-1..1000, -1..1000, -1..1000, -1..1000, -1..1000, -1..1000, -1..1000, -1..1000>>)
ASSUME PrintT("#exact80") /\ PrintT(<<11111111, 11111111, 11111111, 11111111, 11111111, 11111111, 11111111, 111111>>)
ASSUME PrintT("#exact81") /\ PrintT(<<11111111, 11111111, 11111111, 11111111, 11111111, 11111111, 11111111, 1111111>>)
====

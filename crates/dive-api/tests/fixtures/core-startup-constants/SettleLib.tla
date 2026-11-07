---- MODULE SettleLib ----
\* Operators whose bound `n` shares its name with a variable of `Settle`.
LOCAL INSTANCE Naturals
Seqs(S, k) == UNION {[0..(n - 1) -> S] : n \in 1..k}
Failing(k) == {n \in 0..k : IF n = k - 1 THEN n \div 0 = 1 ELSE FALSE}
====

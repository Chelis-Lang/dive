---- MODULE trace_0 ----
LOCAL INSTANCE TLC

CounterExample == 
	[action |-> {<<<<1, [zeta |-> 0, alpha |-> "s", mid |-> (1 :> M @@ 3 :> M)]>>, [name |-> "A", location |-> [beginLine |-> 6, beginColumn |-> 6, endLine |-> 6, endColumn |-> 61, module |-> "Ord"]], <<2, [zeta |-> 1, alpha |-> "s", mid |-> (1 :> M @@ 3 :> M)]>>>>, <<<<2, [zeta |-> 1, alpha |-> "s", mid |-> (1 :> M @@ 3 :> M)]>>, [name |-> "B", location |-> [beginLine |-> 7, beginColumn |-> 6, endLine |-> 7, endColumn |-> 54, module |-> "Ord"]], <<3, [zeta |-> 5, alpha |-> "s", mid |-> (1 :> M @@ 3 :> M)]>>>>}, state |-> {<<1, [zeta |-> 0, alpha |-> "s", mid |-> (1 :> M @@ 3 :> M)]>>, <<2, [zeta |-> 1, alpha |-> "s", mid |-> (1 :> M @@ 3 :> M)]>>, <<3, [zeta |-> 5, alpha |-> "s", mid |-> (1 :> M @@ 3 :> M)]>>}]

====
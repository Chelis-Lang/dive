-------------------------- MODULE FramedFunctions --------------------------
(***************************************************************************)
(* Finite functions with a prescribed value on each unchanged coordinate.  *)
(* Supply an old value for every fixed coordinate.                         *)
(***************************************************************************)
FramedFunctions(keys, values, old, writable) ==
    LET free == keys \cap writable
        fixed == keys \ writable
    IN IF keys # {} /\ values = {}
       THEN {}
       ELSE IF \A k \in fixed : old[k] \in values
            THEN { [k \in keys |-> IF k \in free THEN change[k] ELSE old[k]]
                     : change \in [free -> values] }
            ELSE {}
=============================================================================

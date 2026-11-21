(* Each natural number is a position on one infinite execution, including
   stuttering steps. A chosen action may depend on the state at that position.
   Weak fairness applies to that action only while it stays enabled. *)
From Coq Require Import Arith.Wf_nat Lia Logic.Classical_Prop.

Section FairDrain.
  Context {Action : Type}.
  Variable prospective : nat -> Prop.
  Variable rank : nat -> nat.
  Variables selected enabled fires : Action -> nat -> Prop.

  Hypothesis choose_action : forall i, prospective i -> exists a, selected a i.
  Hypothesis ready : forall a i, selected a i -> enabled a i.
  Hypothesis nonincrease : forall i, prospective i -> prospective (S i) ->
    rank (S i) <= rank i.
  Hypothesis same_rank_keeps_action : forall a i,
    prospective i -> prospective (S i) -> rank (S i) = rank i ->
    selected a i -> selected a (S i).
  Hypothesis firing_decreases : forall a i,
    prospective i -> prospective (S i) -> selected a i -> fires a i ->
    rank (S i) < rank i.
  Hypothesis weak_fairness : forall a i,
    (forall j, i <= j -> enabled a j) -> exists j, i <= j /\ fires a j.

  Theorem eventually_exits : forall i,
    exists j, i <= j /\ ~ prospective j.
  Proof.
    intros i. apply NNPP. intros no_exit.
    assert (stuck : forall j, i <= j -> prospective j).
    { intros j after. apply NNPP. intro outside.
      apply no_exit. exists j. auto. }
    assert (cannot_stay : forall k j, i <= j -> rank j = k -> False).
    { intro k. induction k as [k smaller] using lt_wf_ind.
      intros j after start_rank.
      destruct (classic (exists t, j <= t /\ rank t < k)) as [drop | no_drop].
      - destruct drop as [t [later lower]]. apply (smaller (rank t) lower t); lia.
      - assert (flat : forall t, j <= t -> rank t = k).
        { intros t from_j. induction from_j as [|t from_j ih].
          - exact start_rank.
          - assert (upper : rank (S t) <= rank t) by
              (apply nonincrease; apply stuck; lia).
            assert (lower : ~ rank (S t) < k) by
              (intro lt; apply no_drop; exists (S t); split; lia).
            lia. }
        destruct (choose_action j (stuck j after)) as [a chosen].
        assert (stays_chosen : forall t, j <= t -> selected a t).
        { intros t from_j. induction from_j as [|t from_j ih].
          - exact chosen.
          - apply same_rank_keeps_action; try (apply stuck; lia).
            + rewrite (flat t) by lia. apply flat. lia.
            + exact ih. }
        destruct (weak_fairness a j) as [t [later fired]].
        { intros t later. apply ready. apply stays_chosen. exact later. }
        assert (lt : rank (S t) < rank t).
        { apply (firing_decreases a); try (apply stuck; lia).
          - apply stays_chosen. exact later.
          - exact fired. }
        rewrite (flat t), (flat (S t)) in lt by lia. lia. }
    apply (cannot_stay (rank i) i); lia.
  Qed.
End FairDrain.

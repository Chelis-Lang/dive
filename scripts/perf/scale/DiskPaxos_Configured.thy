theory DiskPaxos_Configured imports DiskPaxos_Invariant begin

text \<open>The Examples read action uses IF; the AFP splits the read
  into alternatives. StartBallot is independently present in Next.\<close>
definition cfg_next :: "state \<Rightarrow> state \<Rightarrow> bool" where
  "cfg_next s t = (\<exists>p.
       StartBallot s t p
     \<or> (\<exists>d. Phase0Read s t p d \<or> Phase1or2Write s t p d
          \<or> (\<exists>q. q \<noteq> p \<and> d \<in> disksWritten s p \<and>
                (if mbal (disk s d q) < mbal (dblock s p)
                 then Phase1or2ReadThen s t p d q else StartBallot s t p)))
     \<or> EndPhase1or2 s t p \<or> Fail s t p \<or> EndPhase0 s t p)"

lemma read_without_restart:
  assumes "\<not> StartBallot s t p"
  shows "(d \<in> disksWritten s p \<and>
          (if mbal (disk s d q) < mbal (dblock s p)
           then Phase1or2ReadThen s t p d q else StartBallot s t p))
         = Phase1or2Read s t p d q"
  using assms by (auto simp: Phase1or2Read_def Phase1or2ReadElse_def Phase1or2ReadThen_def)

lemma same_next: "cfg_next s t = Next s t"
  unfolding cfg_next_def Next_def
  apply (rule arg_cong[where f=Ex], rule ext)
  subgoal for p by (cases "StartBallot s t p"; simp add: read_without_restart)
  done

lemma input_or_sentinel: "(x \<in> Inputs) = (x \<noteq> NotAnInput)"
  by (cases x; simp add: Inputs_def NotAnInput_def)

lemma allowed_input [simp]: "x \<in> Inputs \<union> {NotAnInput}"
  by (simp add: input_or_sentinel)

lemma same_noninitial_blocks:
  "{b \<in> blocksSeen s p. inp b \<noteq> NotAnInput} = nonInitBlks s p"
  by (simp add: nonInitBlks_def input_or_sentinel)

text \<open>HOL enforces the record shapes and total function domains by type.
  The TLA+ record type also bounds both numeric ballot fields. Spell out
  these bounds for every stored or observed block in the configured model.\<close>
definition cfg_block :: "DiskBlock \<Rightarrow> bool" where
  "cfg_block b = (mbal b \<in> AllBallots \<union> {0} \<and>
                  bal b \<in> AllBallots \<union> {0} \<and>
                  inp b \<in> Inputs \<union> {NotAnInput})"

lemma block_of_owner:
  assumes "HInv s" and "b \<in> blocksOf s p"
  shows "cfg_block b"
proof -
  from assms have a: "Inv2a_innermost s p b"
    unfolding HInv_def HInv2_def Inv2a_def Inv2a_inner_def by blast
  have sub: "Ballot p \<subseteq> AllBallots" unfolding AllBallots_def by blast
  from a sub show ?thesis by (auto simp: cfg_block_def Inv2a_innermost_def input_or_sentinel)
qed

lemma read_owner:
  assumes "br \<in> blocksRead s p d"
  shows "block br \<in> blocksOf s (proc br)"
  using assms unfolding blocksOf_def rdBy_def by blast

definition cfg_type :: "state \<Rightarrow> bool" where
  "cfg_type s = (
      (\<forall>p. inpt s p \<in> Inputs \<and> phase s p \<le> 3
           \<and> outpt s p \<in> Inputs \<union> {NotAnInput} \<and> cfg_block (dblock s p))
      \<and> (\<forall>d p. cfg_block (disk s d p)
           \<and> (\<forall>br \<in> blocksRead s p d. cfg_block (block br)))
      \<and> allInput s \<subseteq> Inputs \<and> chosen s \<in> Inputs \<union> {NotAnInput})"

lemma configured_type:
  assumes inv: "HInv s"
  shows "cfg_type s"
proof -
  have d: "cfg_block (dblock s p)" for p
    by (rule block_of_owner[OF inv, where p=p]; auto simp: blocksOf_def)
  have k: "cfg_block (disk s d p)" for d p
    by (rule block_of_owner[OF inv, where p=p]; auto simp: blocksOf_def)
  have r: "br \<in> blocksRead s p d \<Longrightarrow> cfg_block (block br)" for p d br
    by (rule block_of_owner[OF inv, where p="proc br"]; rule read_owner)
  from inv d k r show ?thesis
    by (auto simp: cfg_type_def HInv_def HInv1_def Inv1_def input_or_sentinel)
qed

definition configured_invariants :: "state \<Rightarrow> bool" where
  "configured_invariants s = (cfg_type s \<and> HInv2 s \<and> HInv3 s \<and> HInv4 s \<and> HInv6 s)"

inductive cfg_reachable :: "state \<Rightarrow> bool" where
  init: "HInit s \<Longrightarrow> cfg_reachable s"
| step: "cfg_reachable s \<Longrightarrow> cfg_next s t \<Longrightarrow> HNextPart s t \<Longrightarrow> cfg_reachable t"

lemma strengthened: "cfg_reachable s \<Longrightarrow> HInv s"
  by (induction rule: cfg_reachable.induct) (auto intro: I1 I2 simp: same_next HNext_def)

theorem configured_safety: "cfg_reachable s \<Longrightarrow> configured_invariants s"
  using strengthened configured_type unfolding configured_invariants_def HInv_def by blast

text \<open>A failure can reset process 1 and choose input 1,
  even if that is already its input. Record the specified post-state history.\<close>
theorem fail_enabled: "\<exists>t. cfg_next s t \<and> HNextPart s t"
proof -
  let ?u = "s\<lparr>inpt := (inpt s)(P1 := I1), phase := (phase s)(P1 := 0),
              dblock := (dblock s)(P1 := InitDB), outpt := (outpt s)(P1 := NotAnInput),
              disksWritten := (disksWritten s)(P1 := {}), blocksRead := (blocksRead s)(P1 := (\<lambda>d. {}))\<rparr>"
  let ?v = "(if chosen s \<noteq> NotAnInput \<or> (\<forall>p. outpt ?u p = NotAnInput)
             then chosen s else outpt ?u (SOME p. outpt ?u p \<noteq> NotAnInput))"
  let ?t = "?u\<lparr>chosen := ?v, allInput := allInput s \<union> range (inpt ?u)\<rparr>"
  have f: "Fail s ?t P1" unfolding Fail_def
    by (rule bexI[where x=I1]; simp add: InitializePhase_def Inputs_def)
  have n: "Next s ?t" unfolding Next_def using f by blast
  have h: "HNextPart s ?t" by (simp add: HNextPart_def)
  show ?thesis using n h same_next by blast
qed

end

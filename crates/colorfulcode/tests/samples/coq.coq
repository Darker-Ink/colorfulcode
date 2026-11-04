(* A small development about natural numbers.
   (* Comments nest. *) *)
Require Import Coq.Lists.List.
Import ListNotations.

Inductive tree : Type :=
  | Leaf : tree
  | Node : tree -> nat -> tree -> tree.

Fixpoint size (t : tree) : nat :=
  match t with
  | Leaf => 0
  | Node l _ r => size l + 1 + size r
  end.

Definition double (n : nat) : nat := n + n.

Notation "x ** y" := (x * y) (at level 40, left associativity).

Theorem double_plus : forall n : nat, double n = n + n.
Proof.
  intros n. unfold double. reflexivity.
Qed.

Lemma app_nil : forall (A : Type) (l : list A), l ++ [] = l.
Proof.
  intros A l. induction l as [| x xs IH].
  - reflexivity.
  - simpl. rewrite IH. reflexivity.
Qed.

Record point := mkPoint { px : nat; py : nat }.

Definition origin := mkPoint 0 0.

Definition label : string := "say ""hi""".

Example size_leaf : size Leaf = 0.
Proof. simpl. auto. Qed.

Compute double 21.
Check app_nil.

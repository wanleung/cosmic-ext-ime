--------------------------- MODULE IMELayoutFixed ---------------------------
(*** Fixed policy, evidence-based (smithay sends layout_EFFECTIVE as the     ***)
(*** modifiers `group`: transitional while any modifier is held, equal to    ***)
(*** the locked layout once everything is released).                         ***)
(***                                                                          ***)
(*** Evaluation happens ONLY on settled reports (no key held).  The journal  ***)
(*** traces of Super+Space round trips show settled reports carry the true   ***)
(*** input source and never flap during typing.                              ***)

EXTENDS Naturals

CONSTANT NULL

Groups == {0, 1}

VARIABLES
    userLayout,   \* the user's REAL input source: 0 = English, 1 = Chinese
    compGroup,    \* group the compositor reports in SETTLED events
    enabled,      \* does the IM intercept keys?
    lastGroup     \* last evaluated settled group (NULL = needs resync)

vars == << userLayout, compGroup, enabled, lastGroup >>

Init ==
    /\ userLayout = 1
    /\ compGroup   = 1
    /\ enabled     = TRUE
    /\ lastGroup   = NULL

(* Deliberate source switch: Super+Space.  Folded with the settled report   *)
(* that follows the release.                                                *)
UserSwitch ==
    LET new == 1 - userLayout
    IN
    /\ userLayout' = new
    /\ compGroup' = new
    /\ enabled' = (new = 1)
    /\ lastGroup' = new

(* Fault: a held-modifier report carries a TRANSITIONAL group (smithay      *)
(* layout_effective).  It arrives while keys are held, so it is never       *)
(* settled and never evaluated.  It changes nothing here — that is the      *)
(* point of the model.                                                      *)
TransitionalReport ==
    /\ UNCHANGED << userLayout, compGroup, enabled, lastGroup >>

(* A settled report with no source switch: deduped. *)
SettledReport ==
    /\ lastGroup # NULL
    /\ lastGroup = compGroup
    /\ UNCHANGED << userLayout, compGroup, enabled, lastGroup >>

(* A keymap load (focus change) resets the sync; the compositor follows it  *)
(* with a settled report that re-syncs.  The load itself never decides.     *)
KeymapLoad ==
    /\ lastGroup' = NULL
    /\ UNCHANGED << userLayout, compGroup, enabled >>

(* The settled report after a load, or a spontaneous settled change when    *)
(* the source really switched: this is the only evaluation path.            *)
Evaluate ==
    /\ lastGroup # compGroup
    /\ lastGroup' = compGroup
    /\ enabled' = (compGroup = 1)
    /\ UNCHANGED << userLayout, compGroup >>

Next ==
    \/ UserSwitch
    \/ TransitionalReport
    \/ SettledReport
    \/ KeymapLoad
    \/ Evaluate

Spec == Init /\ [][Next]_vars

NoDisableWhileChinese == enabled = FALSE => userLayout = 0

=============================================================================

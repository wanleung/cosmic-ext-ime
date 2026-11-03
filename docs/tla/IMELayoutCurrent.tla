--------------------------- MODULE IMELayoutCurrent ---------------------------
(*** Model of cosmic-ext-ime's layout-following state machine as shipped in   ***)
(*** 0.2.1+chewing13, with a fault model of the compositor spontaneously      ***)
(*** flipping the reported layout group (observed in the journal: group       ***)
(*** alternates "English (US)"/"Chinese" every 1-4s while the user types,     ***)
(*** without any user input-source switch).                                   ***)
(***                                                                          ***)
(*** Property: the input method must be enabled exactly when the user's       ***)
(*** input source is Chinese.  TLC finds the transitions that violate it.     ***)

EXTENDS Naturals

CONSTANTS NULL

Groups == {0, 1}

VARIABLES
    userLayout,   \* the user's REAL input source: 0 = English, 1 = Chinese
    compGroup,    \* the group the compositor reports in modifiers events
    enabled,      \* does the IM intercept keys?
    lastGroup,    \* last evaluated group (dedupe guard; NULL = none)
    hint,         \* last shift-free reported group (pin)
    shiftHeld,    \* Shift currently depressed?
    flapBudget    \* remaining spontaneous compositor flips (fault budget)

vars == << userLayout, compGroup, enabled, lastGroup, hint, shiftHeld, flapBudget >>

Init ==
    /\ userLayout = 1        \* user is typing Chinese
    /\ compGroup   = 1
    /\ enabled     = TRUE
    /\ lastGroup   = NULL
    /\ hint        = NULL
    /\ shiftHeld   = FALSE
    /\ flapBudget  = 4

(* The user deliberately switches the input source (Super+Space).  The       *)
(* compositor follows: what it reports becomes the real layout.              *)
UserSwitch ==
    LET new == 1 - userLayout
    IN
    /\ userLayout' = new
    /\ compGroup' = new
    /\ enabled' = (new = 1)
    /\ lastGroup' = new
    /\ hint' = new
    /\ UNCHANGED << shiftHeld, flapBudget >>

(* Fault: the compositor flips the group it reports, with no user action.    *)
CompositorFlap ==
    /\ flapBudget > 0
    /\ compGroup' = 1 - compGroup
    /\ flapBudget' = flapBudget - 1
    /\ UNCHANGED << userLayout, enabled, lastGroup, hint, shiftHeld >>

ShiftPress   == /\ shiftHeld' = TRUE  /\ UNCHANGED << userLayout, compGroup, enabled, lastGroup, hint, flapBudget >>
ShiftRelease == /\ shiftHeld' = FALSE /\ UNCHANGED << userLayout, compGroup, enabled, lastGroup, hint, flapBudget >>

(* The compositor delivers a modifiers event reporting compGroup.            *)
(* Transitions follow src/im.rs as of 0.2.1+chewing13 exactly:               *)
(*   1. pin the group on shift-free events                                   *)
(*   2. use the pinned group for xkb state and evaluation                    *)
(*   3. defer evaluation while Shift is effective                            *)
(*   4. skip evaluation when the group is unchanged                          *)
(*   5. otherwise evaluate: enabled := (group is Chinese)                    *)
ModifiersEvent ==
    LET newHint == IF ~shiftHeld THEN compGroup ELSE hint
        g       == IF newHint # NULL THEN newHint ELSE compGroup
    IN
    /\ hint' = newHint
    /\ IF shiftHeld
       THEN UNCHANGED << enabled, lastGroup >>
       ELSE IF lastGroup = g
            THEN UNCHANGED << enabled, lastGroup >>
            ELSE /\ lastGroup' = g
                 /\ enabled' = (g = 1)
    /\ UNCHANGED << userLayout, compGroup, shiftHeld, flapBudget >>

(* A keymap reload resets the guards and evaluates the FRESH xkb state,      *)
(* whose locked group is always 0 ("English (US)").                          *)
KeymapReload ==
    /\ lastGroup' = 0
    /\ hint' = NULL
    /\ enabled' = FALSE
    /\ UNCHANGED << userLayout, compGroup, shiftHeld, flapBudget >>

Next ==
    \/ UserSwitch
    \/ CompositorFlap
    \/ ShiftPress
    \/ ShiftRelease
    \/ ModifiersEvent
    \/ ~TRUE /\ KeymapReload   \* disabled for this run to isolate the flap path


Spec == Init /\ [][Next]_vars

NoDisableWhileChinese == enabled = FALSE => userLayout = 0

=============================================================================

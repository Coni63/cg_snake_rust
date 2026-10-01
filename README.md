# Catch the Rabbits - Game Documentation

## 🎯 The Goal

The primary objective of this game is to catch all the rabbits using your snake.

---

## 📜 Rules

- **Initialization:** On the first turn, your program receives only the coordinates of the rabbits.
- **Game Loop:** On every subsequent turn, you receive the current coordinates of the snake's body parts.
- **Objective:** Catch all rabbits present on the grid.
- **Map Dimensions:** The game map size is **54 x 96** (Height x Width).

### Lose Conditions
You lose the game if:
1. You move outside the boundaries of the map.
2. You do not supply a valid sequence of actions.
3. The snake's head collides with any part of its body.

---

## 🧠 Expert Rules

- The snake must **always be in movement**.
- **Movement Mechanics:**
  - Standard coordinate grid manipulation applies:
    - **Up:** Output `X (Y - 1)`
    - **Down:** Output `X (Y + 1)`
    - **Right:** Output `(X + 1) Y`
    - **Left:** Output `(X - 1) Y`
  - *Example:* If the snake head is at `50 10` and its body extends to `49 10`, `48 10`, `47 10`, moving left would cause a self-collision. You must move up, down, or right. Moving left is only valid if no body part is in that tile.
- **Test Cases:** Test cases may be random or static.

---

## 🏆 Scoring System

Select a target rabbit, move over it, and aim for the highest possible score.

Score calculation upon catching a rabbit:
1. **Penalty:** If you do not catch a rabbit within 10 turns, a penalty applies:
   $$\text{penalty} = \text{turn} \times \text{NB\_TURN\_WITHOUT\_CATCH\_RABBIT}$$
2. **Combo Bonus:** 
   $$\text{add\_combo} = \text{NB\_CATCH\_IN\_2\_TURNS\_MIN} \times 15000$$
   *(Note: `NB_CATCH_IN_2_TURNS_MIN` resets after 2 turns).*
3. **Total Score Update:**
   $$\text{SCORE} = \text{SCORE} + 10000 + \text{add\_combo} - \text{penalty}$$

---

## 📥 Game Input / Output

### Input

#### Initialization Input (Turn 1 only):
- **Line 1:** `N` (An integer representing the total number of rabbits).
- **Next `N` lines:** `X Y` (Two space-separated integers representing the coordinates of each rabbit).

#### Input for Each Game Turn:
- **Line 1:** `NS` (An integer representing the number of snake body parts).
- **Next `NS` lines:** `X Y` (Two space-separated integers representing the coordinates of each snake body part).
  - *Note:* The first `X Y` coordinate in this list is the **snake's head**.

---

### Output

For each turn, output **2 space-separated integers** representing the new coordinates `X Y` for the snake's head.

---

### ⏱️ Constraints

- **Response Time:** $\le 50\text{ ms}$ per turn.
- **Max Turns:** $600$ turns maximum.

---

## 📌 Notes & Tips

- **Testing:** Run your tests locally using the "Test cases" window. You can submit as many times as you like; only your most recent submission counts toward the final ranking.
- **Validation:** Official evaluation test cases differ from training test cases. Avoid hardcoding solutions to ensure a high score.
- **Debugging:** Use the viewer's options to inspect grid movement and state changes during execution.

## How to test:

```
cargo run < testcases/test0X.txt
cargo run --release < testcases/test0X.txt
```
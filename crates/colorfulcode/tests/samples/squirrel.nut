// Entity system for a small game
# shell style comment
/* Block
   comment */
const MAX_ENTITIES = 256;

enum Team { RED, BLUE }

class Entity {
    name = null;
    health = 100;
    static count = 0;

    constructor(n) {
        name = n;
        Entity.count++;
    }

    function damage(amount) {
        health -= amount;
        if (health <= 0) {
            print(format("%s died\n", name));
            return true;
        }
        return false;
    }
}

class Player extends Entity {
    team = Team.RED;
    function toString() { return "Player(" + name + ")"; }
}

local players = [];
foreach (i, label in ["ann", "bob"]) {
    players.append(Player(label));
}
local path = @"C:\games\""data""";
local table = { speed = 1.5, flags = 0xFF, initial = 'x' };
::globalScore <- 0;
local fn = function(p) { return typeof p == "instance" && p instanceof Player; };
print(players.len() + " players, " + fn(players[0]) + "\n");

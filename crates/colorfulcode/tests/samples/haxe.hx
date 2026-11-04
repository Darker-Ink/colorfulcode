package game.entities;

import haxe.ds.StringMap;
using StringTools;

/**
 * A player controlled character.
 */
@:keep
class Player extends Entity implements IDamageable {
    public static inline var MAX_HEALTH:Int = 100;

    public var name(default, null):String;
    var health:Float = 100.0;
    var inventory = new StringMap<Int>();

    public function new(name:String) {
        super();
        this.name = name;
    }

    public function damage(amount:Float):Bool {
        health -= amount;
        if (health <= 0) {
            trace('$name was defeated after ${amount} damage');
            return true;
        }
        return false;
    }

    override function update(dt:Float):Void {
        var pattern = ~/^item_(\d+)$/i;
        for (key in inventory.keys()) {
            if (pattern.match(key)) inventory.set(key, inventory.get(key) + 1);
        }
        #if debug
        trace("updated in " + dt + "s\n");
        #end
    }
}

enum Weapon {
    Sword(damage:Int);
    Bow(range:Float, arrows:Int);
}

typedef Point = { x:Float, y:Float };

class Main {
    static function main() {
        var p = new Player("hero");
        switch (Sword(0x0A)) {
            case Sword(d): p.damage(d * 1.5e1);
            case _:
        }
    }
}

class_name Player
extends CharacterBody2D
## A simple platformer player.

signal health_changed(new_value: int)

enum State { IDLE, RUNNING, JUMPING }

const SPEED := 300.0
const JUMP_VELOCITY = -400.0

@export var max_health: int = 100
@onready var sprite: Sprite2D = $Visuals/Sprite
@onready var hud = %HUD

var state: State = State.IDLE
var health := max_health:
	set(value):
		health = clamp(value, 0, max_health)
		health_changed.emit(health)

func _physics_process(delta: float) -> void:
	if not is_on_floor():
		velocity.y += get_gravity().y * delta
	if Input.is_action_just_pressed(&"jump") and is_on_floor():
		velocity.y = JUMP_VELOCITY
		state = State.JUMPING
	var direction := Input.get_axis("ui_left", "ui_right")
	velocity.x = direction * SPEED if direction else move_toward(velocity.x, 0, SPEED)
	move_and_slide()

func take_damage(amount: int = 10) -> bool:
	health -= amount
	print("Health: %d\n" % health)
	return health <= 0

static func create() -> Player:
	return preload("res://player.tscn").instantiate() as Player

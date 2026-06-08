extends SceneTree

func _initialize() -> void:
	call_deferred("_run")

func _run() -> void:
	var packed: PackedScene = load("res://scenes/master.tscn")
	if packed == null:
		push_error("AUTORUN_E2E failed_to_load_master")
		quit(1)
		return
	var scene := packed.instantiate()
	get_root().add_child(scene)
	current_scene = scene
	for i in range(24):
		await process_frame
		await physics_frame
	var player: CharacterBody3D = scene.get_node_or_null("Player")
	var controller: Node = scene.get_node_or_null("NavigationController")
	if player == null or controller == null:
		push_error("AUTORUN_E2E missing_nodes")
		quit(1)
		return
	var started := bool(controller.call("start_navigation_to_query", "外滩"))
	if not started:
		push_error("AUTORUN_E2E route_start_failed")
		quit(1)
		return
	var hint: Label = controller.get("auto_run_hint_label") as Label
	controller.call("_toggle_auto_run")
	var start_pos := player.global_position
	for i in range(90):
		await physics_frame
	var moved := player.global_position.distance_to(start_pos)
	var enabled := bool(controller.get("auto_run_enabled"))
	var player_auto := bool(player.get("auto_move_enabled"))
	var hint_on := hint != null and hint.visible and hint.text.contains("开启")
	print("AUTORUN_E2E moved=", moved)
	print("AUTORUN_E2E enabled=", enabled)
	print("AUTORUN_E2E player_auto=", player_auto)
	print("AUTORUN_E2E hint_on=", hint_on)
	controller.call("_toggle_auto_run")
	await physics_frame
	var disabled := not bool(controller.get("auto_run_enabled")) and not bool(player.get("auto_move_enabled"))
	var hint_off := hint != null and hint.visible and hint.text.contains("关闭")
	print("AUTORUN_E2E disabled=", disabled)
	print("AUTORUN_E2E hint_off=", hint_off)
	quit(0 if moved > 1.0 and enabled and player_auto and hint_on and disabled and hint_off else 1)

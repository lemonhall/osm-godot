extends SceneTree

func _initialize() -> void:
	call_deferred("_run")

func _run() -> void:
	var packed: PackedScene = load("res://scenes/master.tscn")
	if packed == null:
		push_error("INSPECT_E2E failed_to_load_master")
		quit(1)
		return
	var scene := packed.instantiate()
	get_root().add_child(scene)
	current_scene = scene
	for i in range(24):
		await process_frame
		await physics_frame

	var player: Node3D = scene.get_node_or_null("Player") as Node3D
	var controller: Node = scene.get_node_or_null("NavigationController")
	if player == null or controller == null:
		push_error("INSPECT_E2E missing_player_or_controller")
		quit(1)
		return
	var camera: Camera3D = player.get_node_or_null("Camera3D") as Camera3D
	if camera == null:
		push_error("INSPECT_E2E missing_camera")
		quit(1)
		return

	var forward := -camera.global_transform.basis.z.normalized()
	var right := camera.global_transform.basis.x.normalized()
	var target_marker := Node3D.new()
	target_marker.name = "InspectionTarget_Meta"
	target_marker.set_meta("osm_metadata", {
		"osm_kind": "building",
		"osm_id": "inspect-test-1",
		"name": "塔山测试楼",
		"building": "office",
		"amenity": "library",
		"addr:street": "测试路",
	})
	scene.add_child(target_marker)
	target_marker.global_position = camera.global_position + forward * 30.0

	var distractor_marker := Node3D.new()
	distractor_marker.name = "InspectionDistractor_Meta"
	distractor_marker.set_meta("osm_metadata", {
		"osm_kind": "building",
		"osm_id": "inspect-test-2",
		"name": "误选建筑",
		"building": "retail",
	})
	scene.add_child(distractor_marker)
	distractor_marker.global_position = camera.global_position + forward * 20.0 + right * 30.0
	await process_frame

	controller.set("inspect_display_seconds", 0.05)
	controller.call("_inspect_looked_at_building")
	await process_frame
	var panel: PanelContainer = controller.get("building_inspect_panel") as PanelContainer
	var title: Label = controller.get("building_inspect_title") as Label
	var body: Label = controller.get("building_inspect_body") as Label
	if panel == null or title == null or body == null:
		push_error("INSPECT_E2E missing_inspect_hud")
		quit(1)
		return

	var shown := panel.visible
	var title_ok := title.text.contains("塔山测试楼")
	var body_ok := body.text.contains("建筑类型") and body.text.contains("office") and body.text.contains("OSM ID") and body.text.contains("inspect-test-1")
	var no_distractor := not title.text.contains("误选建筑") and not body.text.contains("inspect-test-2")
	print("INSPECT_E2E shown=", shown)
	print("INSPECT_E2E title=", title.text)
	print("INSPECT_E2E body=", body.text.replace("\n", " | "))
	print("INSPECT_E2E no_distractor=", no_distractor)
	if not (shown and title_ok and body_ok and no_distractor):
		push_error("INSPECT_E2E visible_payload_failed")
		quit(1)
		return

	for i in range(20):
		await process_frame
	var hidden_after_timer := not panel.visible
	print("INSPECT_E2E hidden_after_timer=", hidden_after_timer)
	quit(0 if hidden_after_timer else 1)

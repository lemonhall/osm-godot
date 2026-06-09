extends SceneTree

func _initialize() -> void:
	call_deferred("_run")

func _run() -> void:
	var packed: PackedScene = load("res://scenes/master.tscn")
	if packed == null:
		_fail("failed_to_load_master")
		return

	var scene: Node = packed.instantiate()
	get_root().add_child(scene)
	current_scene = scene

	for i in range(24):
		await process_frame
		await physics_frame

	var player: CharacterBody3D = scene.get_node_or_null("Player")
	if player == null:
		_fail("missing_player")
		return
	var controller: Node = scene.get_node_or_null("NavigationController")
	if controller == null:
		_fail("missing_navigation_controller")
		return
	controller.call("_toggle_panel")
	await process_frame
	var panel: Control = controller.get_node_or_null("NavigationHUD/NavigationPanel")
	var controls_disabled := bool(player.get("controls_enabled")) == false
	var panel_centered := panel != null and is_equal_approx(panel.anchor_left, 0.5) and is_equal_approx(panel.anchor_top, 0.5)
	var mouse_visible := Input.get_mouse_mode() == Input.MOUSE_MODE_VISIBLE
	print("NAV_E2E panel_visible=", panel != null and panel.visible)
	print("NAV_E2E panel_centered=", panel_centered)
	print("NAV_E2E controls_disabled=", controls_disabled)
	print("NAV_E2E mouse_visible=", mouse_visible)
	if panel == null or not panel.visible or not panel_centered or not controls_disabled or not mouse_visible:
		_fail("navigation_panel_focus_failed")
		return
	controller.call("_cancel_navigation_panel")
	await process_frame
	var controls_restored := bool(player.get("controls_enabled")) == true
	var mouse_captured := Input.get_mouse_mode() == Input.MOUSE_MODE_CAPTURED
	print("NAV_E2E controls_restored=", controls_restored)
	print("NAV_E2E mouse_captured=", mouse_captured)
	var display_is_headless := DisplayServer.get_name() == "headless"
	print("NAV_E2E display_is_headless=", display_is_headless)
	if panel.visible or not controls_restored or (not display_is_headless and not mouse_captured):
		_fail("navigation_panel_cancel_failed")
		return

	controller.call("_toggle_panel")
	await process_frame
	var search_box: LineEdit = controller.get("search_box") as LineEdit
	if search_box == null:
		_fail("missing_search_box")
		return
	var result_list: ItemList = controller.get("result_list") as ItemList
	if result_list == null:
		_fail("missing_result_list")
		return
	search_box.text = "外滩"
	controller.call("_on_search_changed", search_box.text)
	await process_frame
	var start_button: Button = controller.get("start_navigation_button") as Button
	if start_button == null:
		_fail("missing_start_button")
		return
	start_button.emit_signal("pressed")
	await process_frame
	if panel.visible or bool(player.get("controls_enabled")) == false:
		_fail("navigation_ui_start_failed_to_close_panel")
		return

	var ok := str(controller.call("get_navigation_status")) == "routing"
	if not ok:
		_fail("route_start_failed status=" + str(controller.call("get_navigation_status")))
		return

	for i in range(12):
		await process_frame
		await physics_frame

	var graph_nodes := int(controller.call("get_graph_node_count"))
	var graph_edges := int(controller.call("get_graph_edge_count"))
	var waypoint_count := int(controller.call("get_route_waypoint_count"))
	var total_distance := float(controller.call("get_route_total_distance"))
	var instruction := str(controller.call("get_current_instruction"))
	var hud := controller.get_node_or_null("NavigationHUD")
	var overlay := controller.get_node_or_null("RouteOverlay")
	var ribbon := controller.get_node_or_null("RouteOverlay/RouteRibbon")
	var destination_circle := controller.get_node_or_null("RouteOverlay/DestinationCircle")
	var turn_markers := controller.get_node_or_null("RouteOverlay/TurnMarkers")

	print("NAV_E2E graph_nodes=", graph_nodes)
	print("NAV_E2E graph_edges=", graph_edges)
	print("NAV_E2E waypoint_count=", waypoint_count)
	print("NAV_E2E total_distance=", total_distance)
	print("NAV_E2E instruction=", instruction)
	print("NAV_E2E status=", controller.call("get_navigation_status"))
	print("NAV_E2E hud_exists=", hud != null)
	print("NAV_E2E overlay_exists=", overlay != null)
	print("NAV_E2E route_ribbon_exists=", ribbon != null)
	print("NAV_E2E destination_circle_exists=", destination_circle != null)
	print("NAV_E2E destination_circle_visible=", destination_circle != null and destination_circle.visible)
	print("NAV_E2E turn_markers_removed=", turn_markers == null)

	var failed := false
	if graph_nodes <= 0:
		push_error("NAV_E2E graph has no nodes")
		failed = true
	if graph_edges <= 0:
		push_error("NAV_E2E graph has no edges")
		failed = true
	if waypoint_count <= 2:
		push_error("NAV_E2E route should contain more than a direct two-point line")
		failed = true
	if total_distance <= 0.0:
		push_error("NAV_E2E route distance is zero")
		failed = true
	if instruction.is_empty():
		push_error("NAV_E2E instruction is empty")
		failed = true
	if hud == null or overlay == null or ribbon == null or destination_circle == null or not destination_circle.visible or turn_markers != null:
		push_error("NAV_E2E missing navigation HUD or overlay nodes")
		failed = true

	if not failed:
		var destination_position: Vector3 = controller.get("destination_position")
		player.global_position = destination_position
		controller.call("_update_guidance")
		await process_frame
		var arrived_status: bool = str(controller.call("get_navigation_status")) == "arrived"
		var cleared_waypoints: bool = int(controller.call("get_route_waypoint_count")) == 0
		var ribbon_hidden: bool = ribbon == null or not ribbon.visible
		var circle_hidden: bool = destination_circle == null or not destination_circle.visible
		var instruction_label: Label = controller.get("instruction_label") as Label
		var distance_label: Label = controller.get("distance_label") as Label
		var hud_hidden: bool = (instruction_label == null or not instruction_label.visible) and (distance_label == null or not distance_label.visible)
		print("NAV_E2E arrived_status=", arrived_status)
		print("NAV_E2E cleared_waypoints=", cleared_waypoints)
		print("NAV_E2E ribbon_hidden_after_arrival=", ribbon_hidden)
		print("NAV_E2E circle_hidden_after_arrival=", circle_hidden)
		print("NAV_E2E hud_hidden_after_arrival=", hud_hidden)
		if not arrived_status or not cleared_waypoints or not ribbon_hidden or not circle_hidden or not hud_hidden:
			push_error("NAV_E2E arrival clear failed")
			failed = true
		var restarted := bool(controller.call("start_navigation_to_query", "中华艺术宫"))
		await process_frame
		var restart_status: bool = str(controller.call("get_navigation_status")) == "routing"
		var restart_waypoints: int = int(controller.call("get_route_waypoint_count"))
		var ribbon_visible_after_restart: bool = ribbon != null and ribbon.visible and ribbon.mesh != null
		var circle_visible_after_restart: bool = destination_circle != null and destination_circle.visible
		print("NAV_E2E restart_started=", restarted)
		print("NAV_E2E restart_status=", controller.call("get_navigation_status"))
		print("NAV_E2E restart_waypoints=", restart_waypoints)
		print("NAV_E2E ribbon_visible_after_restart=", ribbon_visible_after_restart)
		print("NAV_E2E circle_visible_after_restart=", circle_visible_after_restart)
		if not restarted or not restart_status or restart_waypoints <= 2 or not ribbon_visible_after_restart or not circle_visible_after_restart:
			push_error("NAV_E2E restart navigation failed to redraw route")
			failed = true
		player.global_position = Vector3(30058.888671875, 1.8, -35287.02734375)
		controller.call("_complete_navigation")
		await process_frame
		controller.call("_toggle_panel")
		await process_frame
		search_box.text = "中华艺术宫"
		controller.call("_on_search_changed", search_box.text)
		await process_frame
		var ui_restart_results: int = result_list.item_count if result_list != null else 0
		start_button.emit_signal("pressed")
		await process_frame
		var ui_restart_status: bool = str(controller.call("get_navigation_status")) == "routing"
		var ui_restart_waypoints: int = int(controller.call("get_route_waypoint_count"))
		var ribbon_visible_after_ui_restart: bool = ribbon != null and ribbon.visible and ribbon.mesh != null
		var circle_visible_after_ui_restart: bool = destination_circle != null and destination_circle.visible
		var ui_route_waypoints: Array = controller.get("route_waypoints")
		var ui_route_start_distance: float = INF
		if not ui_route_waypoints.is_empty():
			var ui_route_start: Vector3 = ui_route_waypoints[0]
			ui_route_start_distance = ui_route_start.distance_to(player.global_position)
		print("NAV_E2E ui_restart_results=", ui_restart_results)
		print("NAV_E2E ui_restart_status=", controller.call("get_navigation_status"))
		print("NAV_E2E ui_restart_waypoints=", ui_restart_waypoints)
		print("NAV_E2E ribbon_visible_after_ui_restart=", ribbon_visible_after_ui_restart)
		print("NAV_E2E circle_visible_after_ui_restart=", circle_visible_after_ui_restart)
		print("NAV_E2E ui_route_start_distance=", ui_route_start_distance)
		if ui_restart_results <= 0 or not ui_restart_status or ui_restart_waypoints <= 2 or not ribbon_visible_after_ui_restart or not circle_visible_after_ui_restart or ui_route_start_distance > 2.0:
			push_error("NAV_E2E UI restart navigation failed to redraw route")
			failed = true

	quit(1 if failed else 0)

func _fail(reason: String) -> void:
	push_error("NAV_E2E " + reason)
	quit(1)

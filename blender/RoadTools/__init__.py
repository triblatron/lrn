# ~ bl_info = {
    # ~ "name": "Road Tools",
    # ~ "author": "Tony Horrobin",
    # ~ "version": (1, 0, 0),
    # ~ "blender": (5, 0, 0),
    # ~ "location": "View3D > Sidebar > Road Tools",
    # ~ "description": "Designs roads according to highway standards",
    # ~ "category": "Object",
# ~ }

import bpy
import sys
import os
import site

sys.path.append(os.path.dirname(__file__))
import Clothoids

class OBJECT_OT_create_straight(bpy.types.Operator):
    bl_idname = "object.create_straight"
    bl_label = "Create a straight road"

    def execute(self, context):
        print("Hello from my add-on!")
        return {'FINISHED'}
    
class VIEW3D_PT_road_tools_panel(bpy.types.Panel):
    bl_label = "Road Tools"
    bl_idname = "VIEW3D_PT_road_tools"
    bl_space_type = 'VIEW_3D'
    bl_region_type = 'UI'
    bl_category = "Road Tools"   # ← this becomes a new tab in the N-panel

    def draw(self, context):
        self.layout.operator("object.create_straight")
        
def register():
    bpy.utils.register_class(OBJECT_OT_create_straight)
    bpy.utils.register_class(VIEW3D_PT_road_tools_panel)

def unregister():
    bpy.utils.unregister_class(OBJECT_OT_create_straight)
    bpy.utils.unregister_class(VIEW3D_PT_road_tools_panel)

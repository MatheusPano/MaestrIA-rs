import sys
from fontTools.ttLib import TTFont
from fontTools import subset
SRC=sys.argv[3] if len(sys.argv) > 3 else '/opt/maestria/data/flutter_assets/fonts/MaterialIcons-Regular.otf'
names="""space_dashboard_outlined add settings_outlined create_new_folder_outlined history_outlined speed_outlined view_sidebar_outlined search close_rounded filter_list_rounded filter_list_off_rounded chevron_right expand_more hexagon_outlined more_horiz more_vert folder_outlined folder_rounded call_split link_off link open_in_new drive_file_rename_outline folder_off_outlined track_changes bolt sticky_note_2_outlined task_alt bedtime_outlined circle grid_view_rounded grid_off clear_all article_outlined tune terminal account_tree_outlined description_outlined add_circle_outline apps_outlined content_copy content_paste circle_outlined check_circle check_circle_outline refresh play_arrow checklist_rtl chat_bubble_outline push_pin push_pin_outlined extension_outlined close folder_open delete_outline home_outlined cleaning_services_outlined code assignment_outlined remove_circle_outline inbox_outlined check_box check_box_rounded check_box_outline_blank check_box_outline_blank_rounded unfold_more info_outline notifications_outlined notifications_none_outlined notifications_off_outlined notifications_off_rounded done_all_rounded clear_all_rounded expand_more_rounded question_mark_rounded lock_rounded check_rounded lightbulb_outline_rounded chevron_right_rounded priority_high_rounded palette_outlined keyboard_outlined rocket_launch_outlined edit_outlined undo remove block cloud_off_outlined light_mode_outlined check keyboard_arrow_up keyboard_arrow_down arrow_right_alt subdirectory_arrow_right add_comment_outlined swap_horiz monitor_heart_outlined bar_chart dns_outlined storage_outlined bug_report_outlined inventory_2_outlined warning_amber_rounded help_outline replay logout file_open_outlined receipt_long_outlined restart_alt arrow_upward arrow_downward hexagon""".split()
f=TTFont(SRC); cm=f.getBestCmap(); inv={}
for k,v in cm.items(): inv.setdefault(v,k)
out=[]; cps=[]
for n in names:
    g = n if n.endswith(('_outlined','_rounded','_sharp')) else n+'_baseline'
    if g not in inv:
        print('MISSING',n,file=sys.stderr); continue
    out.append((n,inv[g])); cps.append(inv[g])
opts=subset.Options(); opts.layout_features=[]; opts.name_IDs=['*']; opts.notdef_outline=True
s=subset.Subsetter(opts); s.populate(unicodes=cps+list(range(0x20,0x7f))); s.subset(f)
f.save(sys.argv[1])
with open(sys.argv[2],'w') as o:
    o.write('use crate::chrome::icon::Icon;\n\n')
    for n,c in out:
        o.write(f"pub const {n.upper()}: Icon = Icon('\\u{{{c:x}}}');\n")

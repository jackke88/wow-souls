# Exports the tables the world importer reads from the throwaway vMaNGOS server into
# cache\mangos\tsv\<name>.tsv (tab separated, backslash escapes, a header line, NULL as \N).
# Starts the server if it is not up and leaves it up (tools\mangos-db.ps1 stop stops it).
#
# Each file is one `SELECT <header> UNION ALL SELECT CAST(col AS CHAR)... INTO OUTFILE` so every value
# is text, the first line names the columns, a tab / newline / backslash inside a value is written as
# a backslash before it, and SQL NULL is the two characters \N. The server writes the file itself, so
# the statement fails when the file exists (the loop deletes it first).
#
#   tools\export-mangos.ps1                              every table
#   tools\export-mangos.ps1 -Only creature_movement      just the named tables (comma separated)

param([string[]]$Only = @())

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$db = Join-Path $PSScriptRoot 'mangos-db.ps1'
$out = Join-Path $repo 'cache\mangos\tsv'
New-Item -ItemType Directory -Force $out | Out-Null
& $db start
if ($LASTEXITCODE -ne 0) { throw 'could not start the database copy' }

# INTO OUTFILE needs the server started with --secure-file-priv=<this directory> (mangos-db.ps1 start does
# that). A server left running from an older start has it NULL: restart it once.
$priv = (& $db query "SELECT @@secure_file_priv" | Select-Object -Skip 1 | Select-Object -First 1)
# The mysql client prints a backslash as two (C:\\Spel\\...\\tsv\\); compare as lower-case paths with single slashes.
function Normalize-Dir([string]$p) { ($p -replace '[\\/]+', '/').TrimEnd('/').ToLowerInvariant() }
if (-not $priv -or $priv -eq 'NULL' -or (Normalize-Dir $priv) -ne (Normalize-Dir $out)) {
    Write-Host "restarting mysqld: secure_file_priv is '$priv'"
    & $db stop
    Start-Sleep -Seconds 2
    & $db start
    if ($LASTEXITCODE -ne 0) { throw 'could not restart the database copy' }
}

$outFwd = $out -replace '\\', '/'
$live = 'patch_min <= 10 AND patch_max >= 10'
# Newest row (highest patch <= 10) per key of a table keyed (key, patch); `t` is the alias the loop adds.
function Latest($table, $key) {
    "mangos.$table t WHERE t.patch = (SELECT MAX(x.patch) FROM mangos.$table x WHERE x.$key = t.$key AND x.patch <= 10)"
}
# The rest of a statement for a plain table (the loop adds the alias `t` after the table name).
function Live($table, $extra = '') {
    "mangos.$table t WHERE $live$extra"
}

# name -> (column list, FROM ... WHERE ... clause). The header line is unioned in by the loop below.
$tables = [ordered]@{
    creature = @(
        'guid,id,id2,id3,id4,id5,map,position_x,position_y,position_z,orientation,spawntimesecsmin,spawntimesecsmax,wander_distance,movement_type,spawn_flags',
        (Live 'creature' ' AND map = 0'))
    creature_template = @(
        'entry,name,subname,level_min,level_max,faction,npc_flags,display_id1,display_id2,display_id3,display_id4,display_scale1,display_scale2,display_scale3,display_scale4,display_probability1,display_probability2,display_probability3,display_probability4,type,rank,detection_range,call_for_help_range,leash_range,speed_walk,speed_run,loot_id,equipment_id,vendor_id,trainer_id,trainer_type,trainer_class,movement_type,inhabit_type,civilian,flags_extra,health_multiplier,damage_multiplier,unit_class',
        (Latest 'creature_template' 'entry'))
    creature_questrelation = @('id,quest', (Live 'creature_questrelation'))
    creature_involvedrelation = @('id,quest', (Live 'creature_involvedrelation'))
    gameobject_questrelation = @('id,quest', (Live 'gameobject_questrelation'))
    gameobject_involvedrelation = @('id,quest', (Live 'gameobject_involvedrelation'))
    quest_template = @(
        'entry,patch,Method,ZoneOrSort,MinLevel,QuestLevel,Type,RequiredClasses,RequiredRaces,RequiredSkill,RequiredSkillValue,SuggestedPlayers,LimitTime,QuestFlags,SpecialFlags,PrevQuestId,NextQuestId,ExclusiveGroup,BreadcrumbForQuestId,NextQuestInChain,SrcItemId,SrcItemCount,SrcSpell,Title,Details,Objectives,OfferRewardText,RequestItemsText,EndText,ObjectiveText1,ObjectiveText2,ObjectiveText3,ObjectiveText4,ReqItemId1,ReqItemId2,ReqItemId3,ReqItemId4,ReqItemCount1,ReqItemCount2,ReqItemCount3,ReqItemCount4,ReqSourceId1,ReqSourceId2,ReqSourceId3,ReqSourceId4,ReqSourceCount1,ReqSourceCount2,ReqSourceCount3,ReqSourceCount4,ReqCreatureOrGOId1,ReqCreatureOrGOId2,ReqCreatureOrGOId3,ReqCreatureOrGOId4,ReqCreatureOrGOCount1,ReqCreatureOrGOCount2,ReqCreatureOrGOCount3,ReqCreatureOrGOCount4,ReqSpellCast1,ReqSpellCast2,ReqSpellCast3,ReqSpellCast4,RewChoiceItemId1,RewChoiceItemId2,RewChoiceItemId3,RewChoiceItemId4,RewChoiceItemId5,RewChoiceItemId6,RewChoiceItemCount1,RewChoiceItemCount2,RewChoiceItemCount3,RewChoiceItemCount4,RewChoiceItemCount5,RewChoiceItemCount6,RewItemId1,RewItemId2,RewItemId3,RewItemId4,RewItemCount1,RewItemCount2,RewItemCount3,RewItemCount4,RewXP,RewOrReqMoney,RewMoneyMaxLevel,RewSpell,RewSpellCast,PointMapId,PointX,PointY,PointOpt,StartScript,CompleteScript,MaxLevel,RequiredCondition,RepObjectiveFaction,RepObjectiveValue,RequiredMinRepFaction,RequiredMinRepValue,RequiredMaxRepFaction,RequiredMaxRepValue,RewRepFaction1,RewRepFaction2,RewRepFaction3,RewRepFaction4,RewRepFaction5,RewRepValue1,RewRepValue2,RewRepValue3,RewRepValue4,RewRepValue5,RewMailTemplateId,RewMailDelaySecs,RewMailMoney',
        (Latest 'quest_template' 'entry'))
    item_template = @(
        'entry,class,subclass,name,display_id,quality,flags,buy_price,sell_price,inventory_type,allowable_class,allowable_race,item_level,required_level,max_count,stackable,delay,dmg_min1,dmg_max1,dmg_type1,block,armor,spellid_1,spelltrigger_1,start_quest,bonding,page_text,stat_type1,stat_value1,stat_type2,stat_value2,stat_type3,stat_value3,stat_type4,stat_value4,stat_type5,stat_value5,stat_type6,stat_value6,stat_type7,stat_value7,stat_type8,stat_value8,stat_type9,stat_value9,stat_type10,stat_value10',
        (Latest 'item_template' 'entry'))
    gameobject_template = @(
        'entry,type,displayId,name,faction,flags,size,data0,data1,data2,data3,data4,data5,data6,data7,data8,data9',
        (Latest 'gameobject_template' 'entry'))
    gameobject = @(
        'guid,id,map,position_x,position_y,position_z,orientation,spawntimesecsmin,spawntimesecsmax,state,spawn_flags',
        (Live 'gameobject' ' AND map = 0'))
    creature_loot_template = @('entry,item,ChanceOrQuestChance,groupid,mincountOrRef,maxcount', (Live 'creature_loot_template'))
    gameobject_loot_template = @('entry,item,ChanceOrQuestChance,groupid,mincountOrRef,maxcount', (Live 'gameobject_loot_template'))
    reference_loot_template = @('entry,item,ChanceOrQuestChance,groupid,mincountOrRef,maxcount', (Live 'reference_loot_template'))
    npc_vendor = @('entry,slot,item,maxcount,incrtime', 'mangos.npc_vendor t')
    creature_equip_template = @('entry,probability,item1,item2,item3', (Live 'creature_equip_template'))
    # Waypoint paths of the type-2 spawns: `id` is the creature's guid; `waittime` is milliseconds. No patch columns.
    creature_movement = @(
        'id,point,position_x,position_y,position_z,orientation,waittime,wander_distance,script_id,path_id',
        'mangos.creature_movement t')
    creature_movement_template = @(
        'entry,point,position_x,position_y,position_z,orientation,waittime,wander_distance,script_id,path_id',
        'mangos.creature_movement_template t')
    # The scripts a quest runs when it is accepted (`StartScript`) or handed in (`CompleteScript`): `id` is that script id.
    # No patch columns. All columns, all rows (the game does not run them; it logs that it skips them).
    quest_start_scripts = @(
        'id,delay,priority,command,datalong,datalong2,datalong3,datalong4,target_param1,target_param2,target_type,data_flags,dataint,dataint2,dataint3,dataint4,x,y,z,o,condition_id,comments',
        'mangos.quest_start_scripts t')
    quest_end_scripts = @(
        'id,delay,priority,command,datalong,datalong2,datalong3,datalong4,target_param1,target_param2,target_type,data_flags,dataint,dataint2,dataint3,dataint4,x,y,z,o,condition_id,comments',
        'mangos.quest_end_scripts t')
    # `lvl` -> XP needed to go from that level to the next (60 rows; the row of level 60 is never used).
    player_xp_for_level = @('lvl,xp_for_next_level', 'mangos.player_xp_for_level t')
    # Human base attributes per class and level (the game plays one race for now) and each class's base
    # health and mana per level.
    player_levelstats = @('race,class,level,str,agi,sta,inte,spi', 'mangos.player_levelstats t WHERE t.race = 1')
    player_classlevelstats = @('class,level,basehp,basemana', 'mangos.player_classlevelstats t')
    game_event_creature = @('guid,event', 'mangos.game_event_creature t')
    game_event_gameobject = @('guid,event', 'mangos.game_event_gameobject t')
    game_event_quest = @('quest,event', 'mangos.game_event_quest t')
    game_event = @('entry,description,holiday', 'mangos.game_event t')
    # Creature pools: template pools expand an entry to its candidate spawn GUIDs (Hogger/Thuros).
    pool_template = @('entry,max_limit', (Live 'pool_template'))
    pool_creature = @('guid,pool_entry,chance', (Live 'pool_creature'))
    pool_creature_template = @('id,pool_entry,chance', (Live 'pool_creature_template'))
}

# -Only keeps just the named tables (`-File ... -Only a,b` arrives as the one string "a,b": split it).
$Only = @($Only | ForEach-Object { $_ -split ',' } | Where-Object { $_ })
if ($Only.Count -gt 0) {
    $unknown = @($Only | Where-Object { -not $tables.Contains($_) })
    if ($unknown.Count -gt 0) { throw "unknown table(s): $($unknown -join ', ') (have $($tables.Keys -join ', '))" }
    foreach ($k in @($tables.Keys)) { if ($Only -notcontains $k) { $tables.Remove($k) } }
}

# Every column named above must exist; say which ones do not before exporting anything.
$names = ($tables.Keys | ForEach-Object { "'$_'" }) -join ','
$have = @{}
& $db query "SELECT table_name, column_name FROM information_schema.columns WHERE table_schema = 'mangos' AND table_name IN ($names)" |
    Select-Object -Skip 1 | ForEach-Object {
        $p = $_ -split "`t"
        $have["$($p[0]).$($p[1])".ToLowerInvariant()] = $true
    }
$missing = foreach ($name in $tables.Keys) {
    foreach ($c in ($tables[$name][0] -split ',')) {
        if (-not $have.ContainsKey("$name.$c".ToLowerInvariant())) { "$name.$c" }
    }
}
if ($missing) { throw "columns missing in the database: $($missing -join ', ')" }

foreach ($name in $tables.Keys) {
    $cols, $fromSql = $tables[$name]
    $list = $cols -split ','
    $header = ($list | ForEach-Object { "'$_'" }) -join ','
    $select = ($list | ForEach-Object { "CAST(t.``$_`` AS CHAR)" }) -join ','
    $file = Join-Path $out "$name.tsv"
    if (Test-Path $file) { Remove-Item $file }
    # In the SQL text '\t' and '\n' are the tab and newline escapes and '\\' is the one backslash char.
    $sql = "SELECT $header UNION ALL SELECT $select FROM $fromSql INTO OUTFILE '$outFwd/$name.tsv' FIELDS TERMINATED BY '\t' ESCAPED BY '\\' LINES TERMINATED BY '\n'"
    $global:LASTEXITCODE = 0
    & $db query $sql
    if ($LASTEXITCODE -ne 0) { throw "export of $name failed" }
    if (-not (Test-Path $file)) { throw "export of $name wrote no file" }
    # Physical lines; a newline inside a value is written as backslash + newline and counts here too.
    $lines = [System.IO.File]::ReadAllText($file).Split([char]10).Length - 1
    Write-Host ("{0,-28} {1,8} lines" -f $name, $lines)
}

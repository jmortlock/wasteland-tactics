<?xml version="1.0" encoding="UTF-8"?>
<tileset version="1.10" tiledversion="1.10.2" name="wasteland" tilewidth="64" tileheight="64" tilecount="4" columns="4">
 <image source="../tiles/tileset.png" width="256" height="64"/>
 <tile id="0">
  <properties>
   <property name="walkable" type="bool" value="true"/>
  </properties>
 </tile>
 <tile id="1">
  <properties>
   <property name="walkable" type="bool" value="false"/>
   <property name="blocks_sight" type="bool" value="true"/>
  </properties>
 </tile>
 <tile id="2">
  <properties>
   <property name="walkable" type="bool" value="true"/>
   <property name="cover" value="ns"/>
  </properties>
 </tile>
 <tile id="3">
  <properties>
   <property name="walkable" type="bool" value="true"/>
   <property name="cover" value="ew"/>
  </properties>
 </tile>
</tileset>

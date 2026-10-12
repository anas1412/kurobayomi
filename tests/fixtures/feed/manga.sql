-- manga feed fixture (T-50a-035). Window start with TZ=UTC and now 1791460800000 is 1783512000000.
INSERT INTO entries(_id,kind,source,url,title,status,thumbnail_url,favorite,initialized,viewer,item_flags,cover_last_modified,date_added) VALUES(1,'manga',1,'/m1','Alpha',1,'file:///covers/a.jpg',1,1,0,0,5,1000);
INSERT INTO entries(_id,kind,source,url,title,status,thumbnail_url,favorite,initialized,viewer,item_flags,cover_last_modified,date_added) VALUES(2,'manga',1,'/m2','Beta "Quoted"
line ‮RTL',1,NULL,1,1,0,0,0,1000);
INSERT INTO entries(_id,kind,source,url,title,status,thumbnail_url,favorite,initialized,viewer,item_flags,cover_last_modified,date_added) VALUES(3,'manga',1,'/m3','Gamma',1,NULL,0,1,0,0,0,1000);
INSERT INTO entries(_id,kind,source,url,title,status,thumbnail_url,favorite,initialized,viewer,item_flags,cover_last_modified,date_added) VALUES(4,'manga',1,'/m4','Delta',1,NULL,1,1,0,0,0,1000);
INSERT INTO entries(_id,kind,source,url,title,status,thumbnail_url,favorite,initialized,viewer,item_flags,cover_last_modified,date_added) VALUES(5,'manga',1,'/m5','Epsilon',1,NULL,1,1,0,0,0,1000);
INSERT INTO entries(_id,kind,source,url,title,status,thumbnail_url,favorite,initialized,viewer,item_flags,cover_last_modified,date_added) VALUES(6,'manga',1,'/m6','Zeta',1,NULL,1,1,0,0,0,1000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(1,1,'/c1','C1',0,0,0,1.0,0,5000,1783512001000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(2,1,'/c2','C2',0,0,0,2.0,0,4000,1783512002000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(3,2,'/c3','C3',0,0,0,3.0,0,3000,1783512003000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(4,3,'/c4','C4',0,0,0,4.0,0,6000,1783512001000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(5,4,'/c5','C5',0,0,0,5.0,0,2000,1783512000010);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(6,1,'/c6','C6',1,0,0,6.0,0,7000,1783512001000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(7,2,'/c7','C7',0,0,0,7.0,0,8000,1783512000000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(8,2,'/c8','C8',0,0,0,8.0,0,500,1783512001000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(9,5,'/c9','C9',0,0,0,9.0,0,1500,1783512001000);
INSERT INTO items(_id,entry_id,url,name,done,bookmark,progress,number,source_order,date_fetch,date_upload) VALUES(10,6,'/c10','C10',0,0,0,10.0,0,1200,1783512001000);

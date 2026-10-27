# SCD
全称super cd，基于rust，用于实现在终端中快速跳转工作目录  
完成项目后需要加入系统变量，不需要在代码中完成  

# 以下以用户视角描述产品
## scd
直接启动程序，应该弹出以下选择
```
    [c] configure scd
    [a] add new location
    
    [1] <位置1>
    [2] <位置2>
    ···
    [n] <位置n>
```
c用于设置最多显示多少个位置n，并保留后续拓展功能，目前只有一个  
a用于添加一个新的快速跳转地址

数字用于快速跳转到想要去位置  

如果用户使用了c 请逐个询问需要修改的内容
如果用户使用了a 则弹出
```
    type the new location, or type "here" to add the current location
    type "quit" to cancel

    new location:
    <等待用户输入>
```


## scd --help 或者scd -h 
写出用户指南，此处需要交给你

## scd [option]
[option] 是此标题中的所有内容，如果用户记得自己的所有路经的话 直接使用 不需要打开scd

# 产品的结构
编译完成后，用户在以下结构使用scd
```
用户自己管理的位置/scd
-/bin/              # 放置编译好的内容
-/config.json       # 用户的配置 地址，显示多少个位置以及位置信息
```
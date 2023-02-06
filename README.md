# Raven-Engine

**Game engine for learning and practicing.**
Chinese version is down below. 😊

## Cautions

My little toy engine is _NOT_ well documented, so if you want to take deep dive in some crates, please go to check the origin crate.

* raven-reflect: Go to see bevy-reflect.
* raven-render: Go to see kajiya.

## Introduction

Raven-Engine is a experimental sandbox for me to learn various game development techniques, reinforce my programming ability and improve my system design capability.

At my current stage of learning, i am still weak on designing things. I work hard to try to transform my programming ability from `learning` `copying` to `designing` `developing`. However, i am not a genius, i have to learn from others and that's why Raven-Engine is for.

I am a ready-to-graduate college student and still have a lot of interested things to learn. I had wrote some ugly game engines in the past and i think i should learn more from other peoples' projects, instead of pondering on my own and learn nothing. However, this doesn't mean I throw away my own thinking and attempts completely. I would like to try thinking by myself first and learn the pros and cons from other people who had great experience in game development.

### Why Rust?

Though I work on cpp game development, I choose to write this little game engine in Rust. I think learning and writing Rust can also improve my cpp programming skills especially on multi-threading programming. And i think the codes that i have written in Rust can be translate into cpp, you just need to adjust some designs and make it dialect.

## Road Map

### milestone 1

- GPU-Parallel-Ready Render Graph
- Physics (3rd Party)
- Basic Render Features
- Basic Render Pipeline
- Simple User Interface And GUI
- Mesh Skinning
- Asset Management

### milestone 2

- Self-Writing ECS (Entity Component System)
- Job System
- More Advance Render Features
- Self-Writing Simple Physics Library
- Memory Management
- Platform Abstruction
- Sound

## Acknowledgements

This project is mainly learning from:

* [kajiya](https://github.com/EmbarkStudios/kajiya)
* [bevy](https://github.com/bevyengine/bevy)

All projects above are all very fantastic learning material, definitely check those for better documentation.

This repo is just for learning purpose.
No commercial purpose.

## 注意事项

我的小玩具引擎并没有很良好的文档，如果你想深入了解某些功能, 去查看我原本学习的地方能获取更好的文档。

* raven-reflect: 去看 bevy-reflect.
* raven-render: 去看 kajiya.

## 简介

渡鸦引擎是我学习各种游戏开发技术，强化编程能力，提升系统设计能力的实验性沙盒。

在我目前的学习阶段，我在设计方面还很薄弱。我努力尝试将我的编程能力从“学习”“复制”转变为“设计”“开发”。但是，我不是天才，我必须向别人学习，这就是渡鸦引擎的用途。

我是一名即将毕业的大学生，还有很多感兴趣的东西要学。我以前写过一些丑陋的游戏引擎，我觉得我应该多从别人的项目中学习，而不是自己瞎琢磨，然后什么都学不到。但是，这并不意味着我完全抛弃了自己的思考和尝试。我会先尝试自己思考，然后从其他有丰富游戏开发经验的人那里学习利弊，进一步完善自己。

### 为什么使用 Rust

尽管我从事 cpp相关的游戏开发，但我选择用 Rust 编写这个游戏引擎。我认为学习和编写 Rust 也可以提高我的 cpp 编程技能，尤其是在多线程编程方面。而且用 Rust 编写的代码可以翻译成 cpp，你只需要调整一些设计，并使其方言化。

## 致谢

这个 repo 主要从以下工程进行学习：

* [kajiya](https://github.com/EmbarkStudios/kajiya)
* [bevy](https://github.com/bevyengine/bevy)

以上的项目都是很好的学习资料，大家有兴趣强烈推荐自己查看，而且有着更完善的文档。

这个 repo 只是为了学习。
无任何商业用途。
# claude-buddy-dex

独立拆出来的 Rust buddy 撞库和 Web 浏览项目。

![image-20260401214611892](/Users/zeke/workspace/github_work/claude-buddy-dex/assets/image-20260401214611892.png)

## 原理

这个项目按 Claude Code buddy 的生成顺序做确定性计算：

1. `userId + SALT`
2. 计算 32 位 hash
3. 用 `Mulberry32` 作为随机数发生器
4. 依次 roll 出 `rarity`、`species`、`eye`、`hat`、`shiny`、5 个 `stats`

所以同一个 `userId` 会稳定得到同一只 buddy。

当前 Rust 代码实现的是 Node 路径，对应 [src/main.rs](/Users/zeke/workspace/github_work/claude-buddy-dex/src/main.rs) 里的 `hash_string()` FNV-1a 逻辑。

## 状态

- Node 版本规则已完成
- Web 查询和展示已完成
- 仓库已附带精简版 `buddies.db`，一般不需要自己先跑库
- TODO: Bun 版本还没完成

## 使用

### 1. 跑库

不是必须。仓库里已经带了一个可直接用的精简版 [buddies.db](/Users/zeke/workspace/github_work/claude-buddy-dex/buddies.db)。

如果你想继续撞库：

```bash
cargo build --release
./target/release/buddy_bruteforce bruteforce 50000000
```

当数据库已经补满 `P4 7128/7128` 时，程序会直接提示：

```text
已全部覆盖（含 shiny），无需继续！
```

也可以先看当前统计：

```bash
./target/release/buddy_bruteforce stats
```

如果你已经撞到了想要的 `userID`，把它写进 `~/.claude.json` 的 `userID` 字段，确保 `companion` 字段已经删除，然后重启 Claude Code，执行 `/buddy` 重新领取即可。

### 2. 启动 Web

```bash
cd web
python3 server.py
```

默认地址：

```text
http://localhost:3456
```

## 四个阶段

- `P1 90/90`：`物种 × 稀有度`
- `P2 594/594`：`物种 × 稀有度 × 帽子`
- `P3 3564/3564`：`物种 × 稀有度 × 帽子 × 眼睛`
- `P4 7128/7128`：`物种 × 稀有度 × 帽子 × 眼睛 × shiny`

其中：

- `P1` 只关心大类有没有覆盖到
- `P2` 开始区分帽子
- `P3` 把完整外观补齐
- `P4` 再把每种完整外观的 `shiny / 非 shiny` 都补齐

所以通常 `P4` 满了，就说明这个 dex 维度已经完整了。

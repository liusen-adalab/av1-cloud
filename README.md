## 如何使用
**前提条件：** 安装 docker, docker-compose

1. 运行 ./scripts/dev-env.sh 脚本，运行必要的组件服务
    ```
    sh scripts/dev-env.sh
    ```
2. 启动服务
    ```
    cargo run 
    ```
3. 测试
    ```
    curl http://127.0.0.1:5991/ping
    ```

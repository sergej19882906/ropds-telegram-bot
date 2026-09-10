# =============================================================================
# Этап 1: Сборка (Builder)
# =============================================================================
FROM rust:1.82-bookworm AS builder

WORKDIR /app

# Устанавливаем системные зависимости для сборки
RUN apt-get update && \
apt-get install -y --no-install-recommends \
    pkg-config \
    build-essential && \
    rm -rf /var/lib/apt/lists/*

# Копируем весь проект целиком
COPY . .

# Собираем в режиме release
RUN cargo build --release && \
    strip target/release/ropds-telegram-bot

# =============================================================================
# Этап 2: Минимальный образ для запуска (Runtime)
# =============================================================================
FROM debian:bookworm-slim AS runtime

# Устанавливаем только необходимые для работы компоненты
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates tzdata && \
    rm -rf /var/lib/apt/lists/*

# Создаем непривилегированного пользователя для безопасности
RUN groupadd -r bot && useradd -r -g bot -m -d /home/bot bot

WORKDIR /app

# Копируем скомпилированный бинарник из этапа сборки
COPY --from=builder /app/target/release/ropds-telegram-bot /usr/local/bin/

# Назначаем права
RUN chown -R bot:bot /app /home/bot

# Переключаемся на непривилегированного пользователя
USER bot

# Переменные окружения по умолчанию
ENV RUST_LOG=info

# Проверка работоспособности: основной процесс контейнера должен быть запущен.
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD ["sh", "-c", "kill -0 1"]

# Запуск бота
ENTRYPOINT ["ropds-telegram-bot"]
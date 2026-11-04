# syntax=docker/dockerfile:1.7
ARG NODE_VERSION=20

FROM node:${NODE_VERSION}-alpine AS build
WORKDIR /app
ENV NODE_ENV=production \
    PATH="/app/node_modules/.bin:$PATH"

COPY package.json package-lock.json ./
RUN --mount=type=cache,target=/root/.npm \
    npm ci --omit=dev && \
    echo "installed $(ls node_modules | wc -l) packages"

COPY . .
RUN npm run build

RUN <<EOT
set -eux
if [ -f dist/index.js ]; then echo "ok"; fi
EOT

FROM nginx:1.27-alpine
LABEL org.opencontainers.image.title="web" \
      maintainer="ops@example.com"
COPY --from=build /app/dist /usr/share/nginx/html
EXPOSE 80/tcp
USER nginx
HEALTHCHECK --interval=30s --timeout=3s CMD wget -qO- http://localhost/ || exit 1
ONBUILD RUN echo "child image"
ENTRYPOINT ["nginx", "-g", "daemon off;"]
CMD echo "fallback $HOSTNAME"

#include "gui.h"

#include <GLFW/glfw3.h>
#include <imgui.h>
#include <imgui_impl_glfw.h>
#include <imgui_impl_opengl3.h>

#include <cstdlib>
#include <filesystem>
#include <string>

#include "logger.h"

static std::string resolve_asset_path(const std::string& asset_name) {
    std::filesystem::path p1 = std::filesystem::path("../data") / asset_name;
    if (std::filesystem::exists(p1))
        return p1.string();
    std::filesystem::path p2 = std::filesystem::path("data") / asset_name;
    if (std::filesystem::exists(p2))
        return p2.string();

    std::filesystem::path xdg_data_home;
    if (const char* env_xdg = std::getenv("XDG_DATA_HOME")) {
        xdg_data_home = env_xdg;
    } else if (const char* env_home = std::getenv("HOME")) {
        xdg_data_home = std::filesystem::path(env_home) / ".local" / "share";
    }

    if (!xdg_data_home.empty()) {
        std::filesystem::path user_path = xdg_data_home / "rlshim" / "data" / asset_name;
        if (std::filesystem::exists(user_path))
            return user_path.string();
    }

    std::vector<std::filesystem::path> global_paths = {
        std::filesystem::path("/usr/share/rlshim/data") / asset_name,
        std::filesystem::path("/usr/local/share/rlshim/data") / asset_name,
        std::filesystem::path("/opt/rlshim/data") / asset_name,
        std::filesystem::path("/app/share/rlshim/data") / asset_name};

    for (const auto& path : global_paths) {
        if (std::filesystem::exists(path))
            return path.string();
    }

    return asset_name;
}

#define STBI_ONLY_PNG
#define STB_IMAGE_IMPLEMENTATION
#include "stb_image.h"

static GLuint load_texture_from_file(const char* filename, int* out_width, int* out_height) {
    int image_width = 0;
    int image_height = 0;
    unsigned char* image_data = stbi_load(filename, &image_width, &image_height, NULL, 4);
    if (image_data == NULL)
        return 0;

    GLuint image_texture;
    glGenTextures(1, &image_texture);
    glBindTexture(GL_TEXTURE_2D, image_texture);

    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);

#if defined(GL_UNPACK_ROW_LENGTH) && !defined(__EMSCRIPTEN__)
    glPixelStorei(GL_UNPACK_ROW_LENGTH, 0);
#endif
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA, image_width, image_height, 0, GL_RGBA, GL_UNSIGNED_BYTE, image_data);
    stbi_image_free(image_data);

    *out_width = image_width;
    *out_height = image_height;
    return image_texture;
}

static void glfw_error_callback(int error, const char* description) {
    logger::error("GLFW Error {}: {}", error, description);
}

namespace gui {

    struct GUIContext {
        GLFWwindow* window = nullptr;
        GLuint bg_texture = 0;
        int bg_width = 0;
        int bg_height = 0;

        GUIContext(const char* title, int width, int height) {
            glfwSetErrorCallback(glfw_error_callback);
            if (!glfwInit())
                return;

            const char* glsl_version = "#version 130";
            glfwWindowHint(GLFW_CONTEXT_VERSION_MAJOR, 3);
            glfwWindowHint(GLFW_CONTEXT_VERSION_MINOR, 0);
            glfwWindowHint(GLFW_RESIZABLE, GLFW_FALSE);

            window = glfwCreateWindow(width, height, title, nullptr, nullptr);
            if (window == nullptr)
                return;

            glfwMakeContextCurrent(window);
            glfwSwapInterval(1);

            IMGUI_CHECKVERSION();
            ImGui::CreateContext();
            ImGuiIO& io = ImGui::GetIO();
            (void)io;

            ImFont* font = io.Fonts->AddFontFromFileTTF(resolve_asset_path("runescape.ttf").c_str(), 16.0f);
            if (font == nullptr) {
                logger::error("Failed to load custom font!");
            }

            ImGui::StyleColorsDark();

            ImGui_ImplGlfw_InitForOpenGL(window, true);
            ImGui_ImplOpenGL3_Init(glsl_version);

            bg_texture = load_texture_from_file(resolve_asset_path("background.png").c_str(), &bg_width, &bg_height);
            if (bg_texture == 0) {
                logger::error("Failed to load background.png");
            }
        }

        ~GUIContext() {
            if (window) {
                if (bg_texture) {
                    glDeleteTextures(1, &bg_texture);
                }
                ImGui_ImplOpenGL3_Shutdown();
                ImGui_ImplGlfw_Shutdown();
                ImGui::DestroyContext();
                glfwDestroyWindow(window);
                glfwTerminate();
            }
        }

        bool is_valid() const { return window != nullptr; }

        void begin_frame() {
            glfwPollEvents();
            ImGui_ImplOpenGL3_NewFrame();
            ImGui_ImplGlfw_NewFrame();
            ImGui::NewFrame();

            if (bg_texture != 0 && bg_height > 0) {
                ImVec2 display_size = ImGui::GetIO().DisplaySize;
                float scale = display_size.y / (float)bg_height;
                float scaled_w = bg_width * scale;
                float scaled_h = display_size.y;
                ImVec2 p_min = ImVec2((display_size.x - scaled_w) * 0.5f, 0.0f);
                ImVec2 p_max = ImVec2(p_min.x + scaled_w, scaled_h);
                ImGui::GetBackgroundDrawList()->AddImage((ImTextureID)(intptr_t)bg_texture, p_min, p_max);
            }
        }

        void end_frame() {
            ImGui::Render();
            int display_w, display_h;
            glfwGetFramebufferSize(window, &display_w, &display_h);
            glViewport(0, 0, display_w, display_h);
            glClearColor(0.1f, 0.1f, 0.1f, 1.0f);
            glClear(GL_COLOR_BUFFER_BIT);
            ImGui_ImplOpenGL3_RenderDrawData(ImGui::GetDrawData());
            glfwSwapBuffers(window);
        }
    };

    void draw_centered_text(const char* text) {
        float windowWidth = ImGui::GetContentRegionAvail().x;
        float textWidth = ImGui::CalcTextSize(text).x;

        ImGui::SetCursorPosX((windowWidth - textWidth) * 0.5f);
        ImGui::TextUnformatted(text);
    }

    static void open_url_in_browser(const std::string& url) {
        std::string cmd = "xdg-open \"" + url + "\" >/dev/null 2>&1 &";
        system(cmd.c_str());
    }

    std::optional<std::string> prompt_for_url(const std::string& step_name,
                                              const std::string& instructions,
                                              const std::string& instructions_pt_2,
                                              const std::string& url_to_open,
                                              const std::string& url_placeholder) {
        GUIContext ctx(step_name.c_str(), 600, 600);
        if (!ctx.is_valid()) {
            logger::error("Failed to initialize GUI context");
            return std::nullopt;
        }

        std::string result;
        bool submitted = false;
        char input_buf[2048] = "";

        while (!glfwWindowShouldClose(ctx.window) && !submitted) {
            ctx.begin_frame();

            ImGui::SetNextWindowPos(ImVec2(0, 0));
            ImGui::SetNextWindowSize(ImGui::GetIO().DisplaySize);
            ImGui::Begin("Prompt", nullptr,
                         ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoMove |
                             ImGuiWindowFlags_NoBackground);

            ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImFont* quill_font =
                ImGui::GetIO().Fonts->AddFontFromFileTTF(resolve_asset_path("runescape_quill.ttf").c_str());
            ImGui::PushFont(quill_font, 64.0f);
            draw_centered_text("rlshim v1.0");
            ImGui::PopStyleColor();
            ImGui::PopFont();
            ImGui::Dummy(ImVec2(0.0f, ImGui::GetTextLineHeight() * 2.0f));

            ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImGui::TextWrapped("%s", instructions.c_str());
            ImGui::PopStyleColor();
            ImGui::Spacing();
            ImGui::Spacing();

            char output_buf[4096] = "";
            strncpy(output_buf, url_to_open.c_str(), sizeof(output_buf) - 1);
            output_buf[sizeof(output_buf) - 1] = '\0';

            ImGui::PushStyleColor(ImGuiCol_FrameBg, ImVec4(0.0f, 0.0f, 0.0f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_Border, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 1.0f);
            ImGui::InputTextMultiline("##url_to_open", output_buf, IM_ARRAYSIZE(output_buf),
                                      ImVec2(-FLT_MIN, ImGui::GetTextLineHeight() * 8),
                                      ImGuiInputTextFlags_ReadOnly | ImGuiInputTextFlags_WordWrap);
            ImGui::PopStyleColor(2);
            ImGui::PopStyleVar();

            ImGui::PushStyleColor(ImGuiCol_Border, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 1.0f);
            ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(0.25f, 0.25f, 0.25f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_ButtonActive, ImVec4(0.35f, 0.35f, 0.35f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImVec4(0.35f, 0.35f, 0.35f, 1.0f));

            static bool is_opened = false;
            static float open_timer = 0.0f;
            if (is_opened) {
                ImGui::Button("opened!", ImVec2(150, 40));
                open_timer += ImGui::GetIO().DeltaTime;
                if (open_timer > 5.0f) {
                    is_opened = false;
                    open_timer = 0.0f;
                }
            } else {
                if (ImGui::Button("open in browser", ImVec2(150, 40))) {
                    open_url_in_browser(url_to_open);
                    is_opened = true;
                }
            }
            ImGui::SameLine();
            static bool is_copied = false;
            static float copy_timer = 0.0f;
            if (is_copied) {
                ImGui::Button("copied!", ImVec2(150, 40));
                copy_timer += ImGui::GetIO().DeltaTime;
                if (copy_timer > 5.0f) {
                    is_copied = false;
                    copy_timer = 0.0f;
                }
            } else {
                if (ImGui::Button("copy url to clipboard", ImVec2(150, 40))) {
                    ImGui::SetClipboardText(url_to_open.c_str());
                    is_copied = true;
                }
            }

            ImGui::PopStyleColor(4);
            ImGui::PopStyleVar();
            ImGui::Dummy(ImVec2(0, ImGui::GetTextLineHeight()));

            ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImGui::TextWrapped("%s", instructions_pt_2.c_str());
            ImGui::SetNextItemWidth(-FLT_MIN);
            ImGui::PopStyleColor();

            ImGui::PushStyleColor(ImGuiCol_FrameBg, ImVec4(0.0f, 0.0f, 0.0f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_Border, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 1.0f);
            ImGui::InputTextWithHint("##url", url_placeholder.c_str(), input_buf, IM_ARRAYSIZE(input_buf));
            ImGui::PopStyleColor();
            ImGui::PopStyleColor();
            ImGui::PopStyleVar();

            ImGui::Spacing();

            ImGui::PushStyleColor(ImGuiCol_Border, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 1.0f);
            ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(0.25f, 0.25f, 0.25f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_ButtonActive, ImVec4(0.35f, 0.35f, 0.35f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImVec4(0.35f, 0.35f, 0.35f, 1.0f));
            if (ImGui::Button("okay here you go!", ImVec2(120, 40))) {
                result = input_buf;
                submitted = true;
            }
            ImGui::PopStyleColor(4);
            ImGui::PopStyleVar();

            ImGui::End();
            ctx.end_frame();
        }

        if (submitted && !result.empty()) {
            return result;
        }
        return std::nullopt;
    }

    std::optional<auth::game_account> prompt_for_character(const std::vector<auth::game_account>& accounts) {
        if (accounts.empty())
            return std::nullopt;
        if (accounts.size() == 1)
            return accounts[0];

        GUIContext ctx("rlshim - choose your character", 600, 600);
        if (!ctx.is_valid()) {
            logger::error("failed to initialize GUI context");
            return std::nullopt;
        }

        std::optional<auth::game_account> result = std::nullopt;
        bool submitted = false;

        while (!glfwWindowShouldClose(ctx.window) && !submitted) {
            ctx.begin_frame();

            ImGui::SetNextWindowPos(ImVec2(0, 0));
            ImGui::SetNextWindowSize(ImGui::GetIO().DisplaySize);
            ImGui::Begin("Select Character", nullptr,
                         ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoMove |
                             ImGuiWindowFlags_NoBackground);

            ImGui::PushStyleColor(ImGuiCol_Text, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImFont* quill_font =
                ImGui::GetIO().Fonts->AddFontFromFileTTF(resolve_asset_path("runescape_quill.ttf").c_str());
            ImGui::PushFont(quill_font, 64.0f);
            draw_centered_text("rlshim");
            ImGui::PopFont();
            ImGui::Dummy(ImVec2(0.0f, ImGui::GetTextLineHeight()));

            ImGui::PushFont(NULL, 32.0f);
            draw_centered_text("choose a character");
            ImGui::PopFont();
            ImGui::PopStyleColor();
            ImGui::Dummy(ImVec2(0.0f, ImGui::GetTextLineHeight() * 2.0f));

            ImGui::PushStyleColor(ImGuiCol_Button, ImVec4(0.25f, 0.25f, 0.25f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_ButtonActive, ImVec4(0.35f, 0.35f, 0.35f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImVec4(0.35f, 0.35f, 0.35f, 1.0f));
            ImGui::PushStyleColor(ImGuiCol_Border, ImVec4(1.0f, 1.0f, 0.0f, 1.0f));
            ImGui::PushStyleVar(ImGuiStyleVar_FrameBorderSize, 1.0f);

            for (const auto& acc : accounts) {
                float windowWidth = ImGui::GetContentRegionAvail().x;
                float buttonWidth = 300.0f;
                ImGui::SetCursorPosX((windowWidth - buttonWidth) * 0.5f);

                std::string displayName = acc.displayName;
                if (displayName.empty()) {
                    displayName = "[no name set]";
                }

                if (ImGui::Button(displayName.c_str(), ImVec2(buttonWidth, 40))) {
                    result = acc;
                    submitted = true;
                }
                ImGui::Spacing();
            }

            ImGui::PopStyleVar();
            ImGui::PopStyleColor(4);

            ImGui::End();
            ctx.end_frame();
        }

        return result;
    }

}  // namespace gui

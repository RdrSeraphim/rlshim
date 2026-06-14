#include <curl/curl.h>
#include <filesystem>
#include <regex>
#include <string>

#include "curl.h"
#include "logger.h"

#ifdef _WIN32
#define popen _popen
#define pclose _pclose
#endif

namespace runelite {
    std::string runelite_dir = std::string(std::getenv("HOME")) + "/.runelite";
    std::string jar_path = runelite_dir + "/runelite.jar";

    bool establish_jar() {
        if (std::filesystem::exists(jar_path))
            return true;

        CURL* curl = curl_easy_init();
        if (!curl) {
            logger::error("failed to initialize curl to download runelite");
            return false;
        }

        curl_easy_setopt(curl, CURLOPT_URL,
                         "https://github.com/runelite/launcher/releases/latest/download/RuneLite.jar");
        curl_easy_setopt(curl, CURLOPT_FOLLOWLOCATION, 1L);

        FILE* fp = fopen(jar_path.c_str(), "wb");
        if (!fp) {
            logger::error("failed to open runelite.jar for writing");
            curl_easy_cleanup(curl);
            return false;
        }

        curl_easy_setopt(curl, CURLOPT_WRITEDATA, fp);

        CURLcode res = curl_easy_perform(curl);
        fclose(fp);
        curl_easy_cleanup(curl);

        if (res != CURLE_OK) {
            logger::error("failed to download runelite: {}", curl_easy_strerror(res));
            std::filesystem::remove(jar_path);
            return false;
        }

        return std::filesystem::exists(jar_path);
    }

    bool is_valid_java_installed() {
        std::array<char, 128> buf;
        std::string result;

        std::unique_ptr<FILE, int (*)(FILE*)> pipe(popen("java -version 2>&1", "r"), pclose);

        if (!pipe) {
            logger::error("failed to execute java -version");
            return false;
        }

        while (fgets(buf.data(), buf.size(), pipe.get()) != nullptr) {
            result += buf.data();
        }

        std::regex version_regex("version\\s+\"([0-9\\._\\-]+)\"");
        std::smatch match;
        if (std::regex_search(result, match, version_regex)) {
            std::string version_str = match[1];

            // Java 8 and earlier typically output "1.8.x"
            if (version_str.starts_with("1.")) {
                logger::error("java 11+ is required, but found {}", version_str);
                return false;
            }

            // Java 9+ formats it as "11.0.x", "17.0.x", "21"
            size_t dot_pos = version_str.find('.');
            std::string major_str = dot_pos != std::string::npos ? version_str.substr(0, dot_pos) : version_str;

            try {
                int major = std::stoi(major_str);
                if (major >= 11) {
                    return true;
                } else {
                    logger::error("java 11+ is required, but found java {}", major);
                    return false;
                }
            } catch (...) {
                logger::error("failed to parse java version number from: {}", version_str);
                return false;
            }
        }

        logger::error("could not extract java version from string: {}", result);
        return false;
    }

    bool establish_home() {
        if (std::filesystem::exists(runelite_dir))
            return true;
        return std::filesystem::create_directories(runelite_dir);
    }
}  // namespace runelite